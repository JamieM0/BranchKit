//! Real-git test harness — ARCHITECTURE.md §12. Builds actual repos in tempdirs by running git;
//! every parser gets exercised against real git output, not hand-rolled fixtures.

use std::path::{Path, PathBuf};

use branchkit_lib::git::exec::{git, GitError, GitOpts, GitOutput};

pub struct TestRepo {
    dir: tempfile::TempDir,
}

// Each integration test binary compiles this module separately and only uses a subset of these
// helpers, so per-binary dead-code warnings here are noise, not signal.
#[allow(dead_code)]
impl TestRepo {
    /// A fresh repo with `main` as the initial branch and a deterministic committer identity.
    pub async fn init() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = Self { dir };
        repo.run(&["init", "--initial-branch=main", "-q"]).await;
        repo.run(&["config", "user.name", "Test User"]).await;
        repo.run(&["config", "user.email", "test@example.com"])
            .await;
        // Keep byte-for-byte text fixtures deterministic even when the host Git installation
        // defaults to `core.autocrlf=true` (as GitHub's Windows runners do).
        repo.run(&["config", "core.autocrlf", "false"]).await;
        // Deterministic commit dates so tests never depend on wall-clock time.
        repo.run(&["config", "commit.gpgsign", "false"]).await;
        repo
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    fn abs(&self, rel: &str) -> PathBuf {
        self.path().join(rel)
    }

    /// Any git invocation, panicking on failure. Use `try_run` when a nonzero exit (e.g. a
    /// merge conflict) is part of the scenario under test.
    pub async fn run(&self, args: &[&str]) -> GitOutput {
        git(self.path(), args, GitOpts::default())
            .await
            .unwrap_or_else(|e| panic!("git {args:?} failed: {e}"))
    }

    pub async fn try_run(&self, args: &[&str]) -> Result<GitOutput, GitError> {
        git(self.path(), args, GitOpts::default()).await
    }

    pub fn write(&self, rel: &str, content: &str) {
        self.write_bytes(rel, content.as_bytes());
    }

    pub fn write_bytes(&self, rel: &str, content: &[u8]) {
        let path = self.abs(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(path, content).expect("write fixture file");
    }

    pub fn remove(&self, rel: &str) {
        std::fs::remove_file(self.abs(rel)).expect("remove fixture file");
    }

    pub async fn stage(&self, paths: &[&str]) {
        let mut args = vec!["add", "--"];
        args.extend(paths);
        self.run(&args).await;
    }

    pub async fn stage_all(&self) {
        self.run(&["add", "-A"]).await;
    }

    /// Stages everything and commits, returning the new commit's sha.
    pub async fn commit_all(&self, message: &str) -> String {
        self.stage_all().await;
        self.commit(message).await
    }

    /// Commits whatever is currently staged (must be called after an explicit `stage`).
    pub async fn commit(&self, message: &str) -> String {
        self.run(&["commit", "-q", "-m", message]).await;
        self.head_sha().await
    }

    pub async fn head_sha(&self) -> String {
        let output = self.run(&["rev-parse", "HEAD"]).await;
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    pub async fn checkout_new(&self, branch: &str) {
        self.run(&["checkout", "-q", "-b", branch]).await;
    }

    pub async fn checkout(&self, branch_or_sha: &str) {
        self.run(&["checkout", "-q", branch_or_sha]).await;
    }

    pub async fn branch(&self, name: &str) {
        self.run(&["branch", name]).await;
    }

    /// `git merge --no-ff --no-edit <branch>` — always creates a merge commit (never
    /// fast-forwards) so graph-topology fixtures get the shape the test actually asked for.
    /// Returns `Err` on conflict — callers assert on that.
    pub async fn merge(&self, branch: &str) -> Result<GitOutput, GitError> {
        self.try_run(&["merge", "--no-ff", "--no-edit", branch])
            .await
    }

    pub async fn worktree_add(&self, path: &Path, branch: &str) {
        self.run(&[
            "worktree",
            "add",
            "-q",
            path.to_str().expect("utf8 path"),
            branch,
        ])
        .await;
    }
}
