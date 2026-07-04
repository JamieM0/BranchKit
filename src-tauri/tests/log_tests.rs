mod common;

use branchkit_lib::git::log;
use common::TestRepo;

#[tokio::test]
async fn linear_history_topology_and_metadata() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let sha_a = repo.commit_all("first commit").await;
    repo.write("a.txt", "two\n");
    let sha_b = repo.commit_all("second commit\n\nWith a body line.").await;

    let topo = log::topology(repo.path()).await.expect("topology");
    assert_eq!(topo.len(), 2);
    // rev-list --topo-order lists newest first.
    assert_eq!(topo[0].sha, sha_b);
    assert_eq!(topo[0].parents, vec![sha_a.clone()]);
    assert_eq!(topo[1].sha, sha_a);
    assert!(topo[1].parents.is_empty(), "root commit has no parents");

    let metas = log::commit_metadata(repo.path(), &[sha_b.clone(), sha_a.clone()])
        .await
        .expect("metadata");
    assert_eq!(metas.len(), 2);
    assert_eq!(metas[0].sha, sha_b);
    assert_eq!(metas[0].subject, "second commit");
    assert_eq!(metas[0].body, "With a body line.");
    assert_eq!(metas[0].author_name, "Test User");
    assert_eq!(metas[0].author_email, "test@example.com");
    assert_eq!(metas[1].sha, sha_a);
    assert_eq!(metas[1].subject, "first commit");
    assert!(metas[1].body.is_empty());
}

#[tokio::test]
async fn merge_commit_has_both_parents_in_topology() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "base\n");
    let base = repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("b.txt", "feature work\n");
    let feature_tip = repo.commit_all("feature commit").await;

    repo.checkout("main").await;
    repo.write("c.txt", "main work\n");
    let main_tip = repo.commit_all("main commit").await;

    repo.merge("feature").await.expect("clean merge");
    let merge_sha = repo.head_sha().await;

    let topo = log::topology(repo.path()).await.expect("topology");
    let merge_entry = topo.iter().find(|c| c.sha == merge_sha).expect("merge commit present");
    assert_eq!(merge_entry.parents.len(), 2);
    assert!(merge_entry.parents.contains(&main_tip));
    assert!(merge_entry.parents.contains(&feature_tip));

    let base_entry = topo.iter().find(|c| c.sha == base).expect("base commit present");
    assert!(base_entry.parents.is_empty());
}

#[tokio::test]
async fn criss_cross_history_topology() {
    // Two branches that merge into each other in both directions, forming a diamond of merges.
    let repo = TestRepo::init().await;
    repo.write("base.txt", "base\n");
    repo.commit_all("base").await;

    repo.checkout_new("left").await;
    repo.write("left.txt", "left\n");
    let left_tip = repo.commit_all("left commit").await;

    repo.checkout("main").await;
    repo.checkout_new("right").await;
    repo.write("right.txt", "right\n");
    let right_tip = repo.commit_all("right commit").await;

    // Merge left into right, and right (pre-merge state) into left — a criss-cross.
    repo.merge("left").await.expect("merge left into right");
    let right_merge = repo.head_sha().await;

    repo.checkout("left").await;
    repo.merge("right").await.expect("merge right into left, may itself become a merge commit");
    let left_merge = repo.head_sha().await;

    let topo = log::topology(repo.path()).await.expect("topology");
    let right_entry = topo.iter().find(|c| c.sha == right_merge).unwrap();
    assert_eq!(right_entry.parents.len(), 2);
    assert!(right_entry.parents.contains(&right_tip));
    assert!(right_entry.parents.contains(&left_tip));

    let left_entry = topo.iter().find(|c| c.sha == left_merge).unwrap();
    assert_eq!(left_entry.parents.len(), 2);
    assert!(left_entry.parents.contains(&left_tip));
    // second merge pulls in the tip of `right`, which by now is `right_merge`.
    assert!(left_entry.parents.contains(&right_merge));
}

#[tokio::test]
async fn stash_list_reports_base_sha_and_selector() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let base = repo.commit_all("base").await;

    repo.write("a.txt", "two\n");
    repo.run(&["stash", "push", "-m", "wip changes"]).await;

    let stashes = log::stash_list(repo.path()).await.expect("stash list");
    assert_eq!(stashes.len(), 1);
    assert_eq!(stashes[0].base_sha, base);
    assert_eq!(stashes[0].selector, "stash@{0}");
    assert!(stashes[0].subject.contains("wip changes"));
}

#[tokio::test]
async fn stash_list_empty_when_no_stashes() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    let stashes = log::stash_list(repo.path()).await.expect("stash list");
    assert!(stashes.is_empty());
}
