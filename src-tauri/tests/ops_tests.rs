mod common;

use branchkit_lib::git::ops;
use common::TestRepo;

/// The ahead/behind commit previews behind the badge tooltip + fix-it popover (DESIGN_SPEC §4.4)
/// come from `ops::divergence`. Point `feature`'s upstream at `main` (a local ref stands in for a
/// remote so the test needs no network) and assert both directions.
#[tokio::test]
async fn divergence_lists_outgoing_and_incoming_against_upstream() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("f.txt", "f\n");
    repo.commit_all("feature work").await;

    repo.checkout("main").await;
    repo.write("m.txt", "m\n");
    repo.commit_all("main work").await;

    repo.run(&["branch", "--set-upstream-to=main", "feature"]).await;

    let div = ops::divergence(repo.path(), "feature").await.expect("divergence");
    // feature is ahead by its own commit and behind by main's.
    assert_eq!(div.outgoing.len(), 1);
    assert_eq!(div.outgoing[0].subject, "feature work");
    assert_eq!(div.incoming.len(), 1);
    assert_eq!(div.incoming[0].subject, "main work");
}

#[tokio::test]
async fn divergence_is_empty_without_upstream() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;
    let div = ops::divergence(repo.path(), "main").await.expect("divergence");
    assert!(div.outgoing.is_empty());
    assert!(div.incoming.is_empty());
}

/// The drag-merge path (`merge_ref` → `git merge --no-edit`) must produce a real merge commit when
/// the branches diverge — the "drag-merge produces a merge commit" verify item (§4.4).
#[tokio::test]
async fn merge_of_diverged_branches_creates_a_merge_commit() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("f.txt", "f\n");
    repo.commit_all("feature work").await;

    repo.checkout("main").await;
    repo.write("m.txt", "m\n");
    repo.commit_all("main work").await;

    // Exactly what `merge_ref` runs, against the checked-out `main`.
    repo.run(&["merge", "--no-edit", "feature"]).await;

    let head = repo.run(&["rev-list", "--parents", "-n1", "HEAD"]).await;
    let parents = String::from_utf8_lossy(&head.stdout);
    // "<merge> <p1> <p2>" — three tokens means two parents, i.e. a merge commit.
    assert_eq!(parents.trim().split_whitespace().count(), 3);
}

/// Branch delete captures the tip (for the Undo toast) and recreate restores it exactly — §15.13.
#[tokio::test]
async fn delete_then_recreate_restores_branch_at_recorded_sha() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;
    repo.checkout_new("feature").await;
    repo.write("f.txt", "f\n");
    let tip = repo.commit_all("feature work").await;

    repo.checkout("main").await;
    // Record tip, force-delete (feature isn't merged), then recreate — the delete/undo sequence.
    let recorded = repo.run(&["rev-parse", "refs/heads/feature"]).await;
    assert_eq!(String::from_utf8_lossy(&recorded.stdout).trim(), tip);
    repo.run(&["branch", "-D", "--", "feature"]).await;
    assert!(repo.try_run(&["rev-parse", "refs/heads/feature"]).await.is_err());

    repo.run(&["branch", "feature", &tip]).await;
    let restored = repo.run(&["rev-parse", "refs/heads/feature"]).await;
    assert_eq!(String::from_utf8_lossy(&restored.stdout).trim(), tip);
}

/// A local tracking branch created from a remote ref should point at the same commit and track it —
/// the one-gesture remote checkout (DESIGN_SPEC §4.4/§15.1). Exercised here through raw git (the
/// command wrapper needs a Tauri AppHandle) to prove the sequence `checkout -b --track` behaves.
#[tokio::test]
async fn remote_tracking_checkout_sequence_creates_tracking_branch() {
    // Build a "remote" as a second repo and fetch it so refs/remotes/origin/* exist.
    let origin = TestRepo::init().await;
    origin.write("a.txt", "one\n");
    let base = origin.commit_all("base").await;

    let clone = TestRepo::init().await;
    clone
        .run(&["remote", "add", "origin", origin.path().to_str().unwrap()])
        .await;
    clone.run(&["fetch", "-q", "origin"]).await;

    // The remote branch exists but no local `main` tracking it yet in the clone's own namespace.
    clone
        .run(&["checkout", "-b", "main", "--track", "origin/main"])
        .await;

    let head = clone.head_sha().await;
    assert_eq!(head, base);
    let upstream = clone
        .run(&["rev-parse", "--abbrev-ref", "main@{upstream}"])
        .await;
    assert_eq!(
        String::from_utf8_lossy(&upstream.stdout).trim(),
        "origin/main"
    );
}
