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
    assert_eq!(parents.split_whitespace().count(), 3);
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

/// Cherry-pick round trip — the commit row menu's "Cherry-pick commit" (§3.1). Exercised via raw
/// git (the command wrapper needs a Tauri AppHandle) to prove `cherry-pick <sha>` actually applies
/// the target commit's change onto HEAD as a new commit.
#[tokio::test]
async fn cherry_pick_applies_the_commits_change_onto_head() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("f.txt", "feature content\n");
    let feature_sha = repo.commit_all("feature work").await;

    // Advance `main` past the fork point so the cherry-picked commit's parent (main's new tip)
    // differs from the original's parent (base) — otherwise an unchanged main would produce a
    // byte-identical commit object (same tree/parent/author/message), which is correct git
    // behavior but not what this test is trying to demonstrate.
    repo.checkout("main").await;
    repo.write("m.txt", "main work\n");
    repo.commit_all("main work").await;
    repo.run(&["cherry-pick", &feature_sha]).await;

    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
        "feature content\n"
    );
    let subject = repo.run(&["log", "-1", "--pretty=%s"]).await;
    assert_eq!(String::from_utf8_lossy(&subject.stdout).trim(), "feature work");
    // Cherry-pick creates a *new* commit, not a ref move to the original.
    assert_ne!(repo.head_sha().await, feature_sha);
}

/// Revert round trip — the commit row menu's "Revert commit" (§3.1). `git revert --no-edit <sha>`
/// commits the inverse immediately, restoring the file to its pre-commit content.
#[tokio::test]
async fn revert_commits_the_inverse_change_immediately() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    repo.write("a.txt", "two\n");
    let change_sha = repo.commit_all("change a").await;

    repo.run(&["revert", "--no-edit", &change_sha]).await;

    assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "one\n");
    let subject = repo.run(&["log", "-1", "--pretty=%s"]).await;
    assert!(String::from_utf8_lossy(&subject.stdout).contains("Revert"));
}

/// Reset soft/mixed/hard move HEAD to the target while leaving the working tree/index in the
/// state their name promises — the commit row menu's Reset submenu (§3.1/§4.6).
#[tokio::test]
async fn reset_soft_mixed_hard_behave_as_named() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let base = repo.commit_all("base").await;
    repo.write("a.txt", "two\n");
    repo.commit_all("change a").await;

    // Soft: HEAD moves back, the change returns to the index (still staged).
    repo.run(&["reset", "--soft", &base]).await;
    assert_eq!(repo.head_sha().await, base);
    let staged = repo.run(&["diff", "--cached", "--name-only"]).await;
    assert_eq!(String::from_utf8_lossy(&staged.stdout).trim(), "a.txt");

    // Mixed: same HEAD move, but the change is unstaged instead.
    repo.run(&["commit", "-q", "-m", "change a again"]).await;
    repo.run(&["reset", "--mixed", &base]).await;
    assert_eq!(repo.head_sha().await, base);
    let staged = repo.run(&["diff", "--cached", "--name-only"]).await;
    assert!(String::from_utf8_lossy(&staged.stdout).trim().is_empty());
    assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "two\n");

    // Hard: HEAD moves and the working tree reverts too — DESIGN_SPEC §4.6 guards this in the UI.
    repo.run(&["checkout", "--", "a.txt"]).await; // discard the mixed reset's unstaged edit first
    repo.write("a.txt", "three\n");
    repo.commit_all("change a a third time").await;
    repo.run(&["reset", "--hard", &base]).await;
    assert_eq!(repo.head_sha().await, base);
    assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "one\n");
}

/// Lightweight vs annotated tag creation — the commit row menu's "Create tag here" / "Create
/// annotated tag here" (§2.9/§3.1).
#[tokio::test]
async fn creates_lightweight_and_annotated_tags() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let sha = repo.commit_all("base").await;

    repo.run(&["tag", "v1", &sha]).await;
    repo.run(&["tag", "-a", "-m", "Release notes", "v1-annotated", &sha]).await;

    let lightweight_type = repo.run(&["cat-file", "-t", "v1"]).await;
    assert_eq!(String::from_utf8_lossy(&lightweight_type.stdout).trim(), "commit");
    let annotated_type = repo.run(&["cat-file", "-t", "v1-annotated"]).await;
    assert_eq!(String::from_utf8_lossy(&annotated_type.stdout).trim(), "tag");

    repo.run(&["tag", "-d", "v1"]).await;
    assert!(repo.try_run(&["rev-parse", "refs/tags/v1"]).await.is_err());
}
