//! Git execution layer — ARCHITECTURE.md §3. Every other module in `git/` shells out through
//! `git()` in this file; nothing else in the codebase should spawn `git` directly.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

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
        assert_eq!(v, GitVersion { major: 2, minor: 43, patch: 0 });
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
        let v = detect_git_version().await.expect("git must be on PATH for tests");
        assert!(v.major >= 2);
    }
}
