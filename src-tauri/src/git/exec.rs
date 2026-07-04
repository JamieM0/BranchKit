//! Git execution layer — ARCHITECTURE.md §3. Every other module in `git/` shells out through
//! `git()` in this file; nothing else in the codebase should spawn `git` directly.

use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::process::{ChildStderr, Command};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Minimum supported git version — ARCHITECTURE.md §1.
pub const MIN_GIT_VERSION: (u32, u32) = (2, 30);

/// Config flags injected into every invocation — ARCHITECTURE.md §3 rule 2.
const BASE_CONFIG: &[&str] = &[
    "-c",
    "color.ui=false",
    "-c",
    "core.quotepath=false",
    "-c",
    "log.showSignature=false",
];

/// Raw output of a successful git invocation. `stdout` is bytes, never a `String` — filenames
/// and diffs may be non-UTF8; callers must decode lossily only at display boundaries.
#[derive(Debug, Clone)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub code: i32,
}

/// Per-call options. Read commands use `GitOpts::default()` (30s); network mutations
/// (fetch/pull/push/clone) should use `GitOpts::network()` (10min) — ARCHITECTURE.md §3 rule 3.
#[derive(Debug, Clone, Copy)]
pub struct GitOpts {
    pub timeout: Duration,
}

impl Default for GitOpts {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }
}

impl GitOpts {
    pub fn network() -> Self {
        Self {
            timeout: Duration::from_secs(600),
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitErrorKind {
    /// git exited with a nonzero status.
    NonZeroExit,
    /// The command ran longer than its timeout and was killed.
    Timeout,
    /// The `git` process could not be spawned or its I/O failed unexpectedly.
    Spawn,
}

/// ARCHITECTURE.md §3 rule 5: on nonzero exit, wrap into this shape before it reaches the UI
/// (the error translator in §9 sits on top of this and is out of scope for this module).
#[derive(Debug, Clone)]
pub struct GitError {
    pub code: Option<i32>,
    pub stderr: String,
    pub cmd_summary: String,
    pub kind: GitErrorKind,
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            GitErrorKind::Timeout => write!(f, "`{}` timed out", self.cmd_summary),
            GitErrorKind::Spawn => {
                write!(f, "failed to run `{}`: {}", self.cmd_summary, self.stderr)
            }
            GitErrorKind::NonZeroExit => write!(
                f,
                "`{}` exited with code {}: {}",
                self.cmd_summary,
                self.code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                self.stderr
            ),
        }
    }
}

impl std::error::Error for GitError {}

impl GitError {
    fn spawn(cmd_summary: &str, message: impl std::fmt::Display) -> Self {
        Self {
            code: None,
            stderr: message.to_string(),
            cmd_summary: cmd_summary.to_string(),
            kind: GitErrorKind::Spawn,
        }
    }

    fn timeout(cmd_summary: String) -> Self {
        Self {
            code: None,
            stderr: String::new(),
            cmd_summary,
            kind: GitErrorKind::Timeout,
        }
    }

    fn exited(code: i32, stderr: String, cmd_summary: String) -> Self {
        Self {
            code: Some(code),
            stderr,
            cmd_summary,
            kind: GitErrorKind::NonZeroExit,
        }
    }
}

fn build_command(repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .args(BASE_CONFIG)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        // Put the child in its own process group (pgid == its pid) so a timeout kill can take
        // any grandchildren with it via killpg — ARCHITECTURE.md §3 rule 3.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

#[cfg(unix)]
fn kill_process_group(pid: u32) {
    // SAFETY: pid is the id of a child we just spawned into its own process group above, so
    // negating it targets that group and nothing else.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

/// Spawn `git <args>` in `repo`, enforcing the timeout in `opts` and killing the whole process
/// group if it's exceeded. See module docs / ARCHITECTURE.md §3 for the non-negotiable rules.
pub async fn git(repo: &Path, args: &[&str], opts: GitOpts) -> Result<GitOutput, GitError> {
    let cmd_summary = format!("git {}", args.join(" "));
    let mut command = build_command(repo, args);

    let mut child = command
        .spawn()
        .map_err(|e| GitError::spawn(&cmd_summary, e))?;

    let mut stdout_pipe = child.stdout.take().expect("stdout was piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr was piped");

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        stdout_pipe.read_to_end(&mut buf).await.map(|_| buf)
    });
    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        stderr_pipe.read_to_end(&mut buf).await.map(|_| buf)
    });

    let run = async {
        let status = child.wait().await?;
        let stdout = stdout_task
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))??;
        let stderr = stderr_task
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))??;
        Ok::<_, std::io::Error>((status, stdout, stderr))
    };

    match tokio::time::timeout(opts.timeout, run).await {
        Ok(Ok((status, stdout, stderr_bytes))) => {
            let stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();
            let code = status.code().unwrap_or(-1);
            if status.success() {
                Ok(GitOutput {
                    stdout,
                    stderr,
                    code,
                })
            } else {
                Err(GitError::exited(code, stderr, cmd_summary))
            }
        }
        Ok(Err(e)) => Err(GitError::spawn(&cmd_summary, e)),
        Err(_elapsed) => {
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                kill_process_group(pid);
            }
            // Best effort on every platform (also the only mechanism on Windows, which has no
            // POSIX process groups — SPEC-DEVIATION: a job object would guarantee subtree
            // termination there but is out of scope for this read-only layer).
            let _ = child.start_kill();
            Err(GitError::timeout(cmd_summary))
        }
    }
}

/// A parsed `N%` progress line from a network op's stderr — ARCHITECTURE.md §3.1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdate {
    pub phase: String,
    pub percent: u32,
}

/// Matches git's `--progress` phase lines, e.g. `Receiving objects:  42% (420/1000)`.
fn parse_progress_line(line: &str) -> Option<ProgressUpdate> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"^(Counting objects|Compressing objects|Receiving objects|Resolving deltas|Writing objects|Updating files):\s+(\d+)%",
        )
        .expect("static regex is valid")
    });
    let caps = re.captures(line.trim())?;
    Some(ProgressUpdate {
        phase: caps.get(1)?.as_str().to_string(),
        percent: caps.get(2)?.as_str().parse().ok()?,
    })
}

/// Like [`git`], but reads stderr incrementally — split on `\r` as well as `\n`, since progress
/// bars rewrite their line with `\r` — and calls `on_progress` with each parsed percentage
/// (ARCHITECTURE.md §3.1). `cwd` need not be a git repo yet: `clone` runs with `cwd` set to the
/// destination's parent directory, before the repo exists.
pub async fn git_with_progress(
    cwd: &Path,
    args: &[&str],
    opts: GitOpts,
    mut on_progress: impl FnMut(ProgressUpdate),
) -> Result<GitOutput, GitError> {
    let cmd_summary = format!("git {}", args.join(" "));
    let mut command = build_command(cwd, args);

    let mut child = command
        .spawn()
        .map_err(|e| GitError::spawn(&cmd_summary, e))?;

    let mut stdout_pipe = child.stdout.take().expect("stdout was piped");
    let stderr_pipe = child.stderr.take().expect("stderr was piped");

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        stdout_pipe.read_to_end(&mut buf).await.map(|_| buf)
    });

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let stderr_task = tokio::spawn(read_stderr_lines(stderr_pipe, tx));

    let run = async {
        // Drains until the stderr reader drops `tx`, which happens once the pipe hits EOF —
        // i.e. once the child has exited (or closed stderr early).
        while let Some(line) = rx.recv().await {
            if let Some(update) = parse_progress_line(&line) {
                on_progress(update);
            }
        }
        let status = child.wait().await?;
        let stdout = stdout_task
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))??;
        let stderr_bytes = stderr_task
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))??;
        Ok::<_, std::io::Error>((status, stdout, stderr_bytes))
    };

    match tokio::time::timeout(opts.timeout, run).await {
        Ok(Ok((status, stdout, stderr_bytes))) => {
            let stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();
            let code = status.code().unwrap_or(-1);
            if status.success() {
                Ok(GitOutput {
                    stdout,
                    stderr,
                    code,
                })
            } else {
                Err(GitError::exited(code, stderr, cmd_summary))
            }
        }
        Ok(Err(e)) => Err(GitError::spawn(&cmd_summary, e)),
        Err(_elapsed) => {
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                kill_process_group(pid);
            }
            let _ = child.start_kill();
            Err(GitError::timeout(cmd_summary))
        }
    }
}

async fn read_stderr_lines(
    mut pipe: ChildStderr,
    tx: tokio::sync::mpsc::UnboundedSender<String>,
) -> std::io::Result<Vec<u8>> {
    let mut all = Vec::new();
    let mut buf = [0u8; 4096];
    let mut current = String::new();
    loop {
        let n = pipe.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        all.extend_from_slice(&buf[..n]);
        for ch in String::from_utf8_lossy(&buf[..n]).chars() {
            if ch == '\r' || ch == '\n' {
                if !current.is_empty() {
                    let _ = tx.send(std::mem::take(&mut current));
                }
            } else {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        let _ = tx.send(current);
    }
    Ok(all)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    pub fn is_supported(&self) -> bool {
        (self.major, self.minor) >= MIN_GIT_VERSION
    }
}

/// Detect the installed git version (`git --version`). Doesn't need a repo, so it's not routed
/// through `git()` above. Used at startup to show the blocking "install git" screen (§1).
pub async fn detect_git_version() -> Result<GitVersion, GitError> {
    let cmd_summary = "git --version".to_string();
    let mut command = Command::new("git");
    command
        .arg("--version")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let output = tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .map_err(|_| GitError::timeout(cmd_summary.clone()))?
        .map_err(|e| GitError::spawn(&cmd_summary, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(GitError::exited(
            output.status.code().unwrap_or(-1),
            stderr,
            cmd_summary,
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_git_version(&stdout)
        .ok_or_else(|| GitError::spawn(&cmd_summary, format!("unparseable output: {stdout}")))
}

fn parse_git_version(text: &str) -> Option<GitVersion> {
    // e.g. "git version 2.43.0" or "git version 2.43.0.windows.1"
    let rest = text.trim().strip_prefix("git version ")?;
    let mut parts = rest.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some(GitVersion {
        major,
        minor,
        patch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_version() {
        let v = parse_git_version("git version 2.43.0\n").unwrap();
        assert_eq!(
            v,
            GitVersion {
                major: 2,
                minor: 43,
                patch: 0
            }
        );
        assert!(v.is_supported());
    }

    #[test]
    fn parses_windows_suffixed_version() {
        let v = parse_git_version("git version 2.30.1.windows.1").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 30);
        assert!(v.is_supported());
    }

    #[test]
    fn rejects_old_version() {
        let v = parse_git_version("git version 2.20.0").unwrap();
        assert!(!v.is_supported());
    }

    #[tokio::test]
    async fn detects_real_git_version() {
        let v = detect_git_version()
            .await
            .expect("git must be on PATH for tests");
        assert!(v.major >= 2);
    }

    #[test]
    fn parses_receiving_objects_progress_line() {
        let update = parse_progress_line("Receiving objects:  42% (420/1000)").unwrap();
        assert_eq!(update.phase, "Receiving objects");
        assert_eq!(update.percent, 42);
    }

    #[test]
    fn parses_progress_line_after_trimming_cr() {
        let update = parse_progress_line("\rResolving deltas: 100% (10/10), done.").unwrap();
        assert_eq!(update.phase, "Resolving deltas");
        assert_eq!(update.percent, 100);
    }

    #[test]
    fn ignores_non_progress_lines() {
        assert!(parse_progress_line("fatal: repository not found").is_none());
    }

    #[tokio::test]
    async fn git_with_progress_reports_clone_phases_and_returns_output() {
        let src = tempfile::tempdir().expect("tempdir");
        git(
            src.path(),
            &["init", "--initial-branch=main", "-q"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        git(
            src.path(),
            &["config", "user.name", "T"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        git(
            src.path(),
            &["config", "user.email", "t@example.com"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        git(
            src.path(),
            &["config", "commit.gpgsign", "false"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        std::fs::write(src.path().join("f.txt"), "hi").unwrap();
        git(src.path(), &["add", "-A"], GitOpts::default())
            .await
            .unwrap();
        git(
            src.path(),
            &["commit", "-q", "-m", "init"],
            GitOpts::default(),
        )
        .await
        .unwrap();

        let parent = tempfile::tempdir().expect("tempdir");
        let dest = parent.path().join("clone-dest");
        let mut phases = Vec::new();
        let src_str = src.path().to_str().unwrap();
        let dest_str = dest.to_str().unwrap();
        // `--no-local` forces the transport path (rather than a hardlink clone), which is what
        // actually emits `N%` progress lines even for a same-machine source.
        let output = git_with_progress(
            parent.path(),
            &["clone", "--no-local", "--progress", src_str, dest_str],
            GitOpts::default(),
            |update| phases.push(update.phase),
        )
        .await
        .expect("clone should succeed");

        assert_eq!(output.code, 0);
        assert!(dest.join(".git").exists());
        assert!(
            !phases.is_empty(),
            "expected at least one progress phase to be observed"
        );
    }
}
