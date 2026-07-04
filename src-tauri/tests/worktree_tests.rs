mod common;

use branchkit_lib::git::{log, refs, status};
use common::TestRepo;

#[tokio::test]
async fn parsers_work_against_a_linked_worktree() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let sha = repo.commit_all("base").await;
    repo.branch("side").await;

    let worktree_dir = tempfile::tempdir().expect("tempdir");
    let worktree_path = worktree_dir.path().join("side-wt");
    repo.worktree_add(&worktree_path, "side").await;

    // The worktree's `.git` is a file (a pointer back to the main repo), not a directory —
    // every parser must work unchanged when `repo` points into it.
    let head = refs::head_info(&worktree_path)
        .await
        .expect("head_info in worktree");
    assert!(!head.detached);
    assert_eq!(head.branch.as_deref(), Some("side"));
    assert_eq!(head.sha, sha);

    let topo = log::topology(&worktree_path)
        .await
        .expect("topology in worktree");
    assert!(topo.iter().any(|c| c.sha == sha));

    std::fs::write(worktree_path.join("untracked.txt"), "new\n").expect("write in worktree");
    let report = status::status(&worktree_path)
        .await
        .expect("status in worktree");
    assert!(report.entries.iter().any(|e| e.path == "untracked.txt"));
}
