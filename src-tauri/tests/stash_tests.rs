mod common;

use common::TestRepo;

/// Stash push (with message + untracked) then pop restores the working tree exactly —
/// DESIGN_SPEC.md §3.2/§4.5/§15.18. Exercised via raw git (the command wrapper needs a Tauri
/// AppHandle) to prove the exact flag sequence `stash push -u -m <msg>` / `stash pop` round-trips.
#[tokio::test]
async fn stash_push_with_message_and_untracked_then_pop_restores_everything() {
    let repo = TestRepo::init().await;
    repo.write("tracked.txt", "one\n");
    repo.commit_all("base").await;

    repo.write("tracked.txt", "one CHANGED\n");
    repo.write("untracked.txt", "new file\n");

    repo.run(&["stash", "push", "-u", "-m", "my stash"]).await;

    // Working tree is clean again — both files gone from the worktree's uncommitted state.
    assert_eq!(std::fs::read_to_string(repo.path().join("tracked.txt")).unwrap(), "one\n");
    assert!(!repo.path().join("untracked.txt").exists());

    let list = repo.run(&["stash", "list"]).await;
    assert!(String::from_utf8_lossy(&list.stdout).contains("my stash"));

    repo.run(&["stash", "pop"]).await;

    assert_eq!(
        std::fs::read_to_string(repo.path().join("tracked.txt")).unwrap(),
        "one CHANGED\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
        "new file\n"
    );
    let list_after = repo.run(&["stash", "list"]).await;
    assert!(String::from_utf8_lossy(&list_after.stdout).trim().is_empty());
}

/// Apply keeps the stash (unlike pop); drop then removes it — the stash row menu's Apply/Drop
/// (GITKRAKEN_WORKFLOWS.md §3.3).
#[tokio::test]
async fn stash_apply_keeps_the_stash_drop_removes_it() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;
    repo.write("a.txt", "two\n");
    repo.run(&["stash", "push"]).await;

    repo.run(&["stash", "apply"]).await;
    assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "two\n");
    let list = repo.run(&["stash", "list"]).await;
    assert!(!String::from_utf8_lossy(&list.stdout).trim().is_empty(), "apply must not drop the stash");

    repo.run(&["stash", "drop"]).await;
    let list_after = repo.run(&["stash", "list"]).await;
    assert!(String::from_utf8_lossy(&list_after.stdout).trim().is_empty());
}

/// A stash commit's own sha, read through the normal commit-metadata/diff machinery, shows the
/// change against its base — this is what makes clicking a stash row in the graph show sensible
/// contents in the right panel without any stash-specific parsing (ARCHITECTURE.md §5.1).
#[tokio::test]
async fn stash_commit_diffs_against_its_base_like_an_ordinary_commit() {
    use branchkit_lib::git::diff::commit_files;
    use branchkit_lib::git::log::stash_list;

    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;
    repo.write("a.txt", "one changed\n");
    repo.run(&["stash", "push"]).await;

    let stashes = stash_list(repo.path()).await.expect("stash list");
    assert_eq!(stashes.len(), 1);

    let files = commit_files(repo.path(), &stashes[0].sha).await.expect("commit files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "a.txt");
}
