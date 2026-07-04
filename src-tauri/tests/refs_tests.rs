mod common;

use branchkit_lib::git::refs::{self, RefKind};
use common::TestRepo;

#[tokio::test]
async fn lists_branches_and_tags_with_head_marker() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("first").await;
    repo.run(&["tag", "v1.0.0"]).await;
    repo.branch("other").await;

    let refs = refs::list_refs(repo.path()).await.expect("list_refs");

    let main = refs
        .iter()
        .find(|r| r.short_name == "main")
        .expect("main branch");
    assert_eq!(main.kind, RefKind::Branch);
    assert!(main.is_head);

    let other = refs
        .iter()
        .find(|r| r.short_name == "other")
        .expect("other branch");
    assert_eq!(other.kind, RefKind::Branch);
    assert!(!other.is_head);

    let tag = refs.iter().find(|r| r.short_name == "v1.0.0").expect("tag");
    assert_eq!(tag.kind, RefKind::Tag);
}

#[tokio::test]
async fn ahead_behind_computed_from_upstream_track() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let base = repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("feature.txt", "feature\n");
    repo.commit_all("feature commit").await;

    repo.checkout("main").await;
    repo.write("main.txt", "main\n");
    repo.commit_all("main commit").await;
    let _ = base;

    // Point feature's upstream at main (a local ref) so %(upstream:track) has something to diff.
    repo.run(&["branch", "--set-upstream-to=main", "feature"])
        .await;

    let refs = refs::list_refs(repo.path()).await.expect("list_refs");
    let feature = refs
        .iter()
        .find(|r| r.short_name == "feature")
        .expect("feature branch");
    assert_eq!(feature.upstream.as_deref(), Some("main"));
    assert_eq!(feature.ahead, 1, "feature has its own commit main lacks");
    assert_eq!(feature.behind, 1, "feature lacks main's commit");
    assert!(!feature.gone);
}

#[tokio::test]
async fn gone_upstream_reported_after_prune() {
    let remote = TestRepo::init().await;
    remote.write("a.txt", "one\n");
    remote.commit_all("base").await;
    remote
        .run(&["config", "receive.denyCurrentBranch", "updateInstead"])
        .await;

    let clone_dir = tempfile::tempdir().expect("tempdir");
    let repo = TestRepo::init().await;
    repo.run(&[
        "remote",
        "add",
        "origin",
        remote.path().to_str().expect("utf8 path"),
    ])
    .await;
    repo.run(&["fetch", "origin"]).await;
    repo.run(&["checkout", "-q", "-b", "main", "origin/main"])
        .await;
    repo.run(&["branch", "--set-upstream-to=origin/main", "main"])
        .await;
    drop(clone_dir);

    // Delete the branch on the "remote" and prune it away locally.
    remote.checkout_new("temp").await;
    remote.run(&["branch", "-D", "main"]).await;
    repo.run(&["fetch", "--prune", "origin"]).await;

    let refs = refs::list_refs(repo.path()).await.expect("list_refs");
    let main = refs
        .iter()
        .find(|r| r.short_name == "main")
        .expect("main branch");
    assert!(main.gone, "upstream ref was deleted and pruned");
}

#[tokio::test]
async fn head_info_reports_attached_branch() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let sha = repo.commit_all("base").await;

    let head = refs::head_info(repo.path()).await.expect("head_info");
    assert!(!head.detached);
    assert_eq!(head.branch.as_deref(), Some("main"));
    assert_eq!(head.sha, sha);
}

#[tokio::test]
async fn head_info_reports_detached_state() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let sha = repo.commit_all("base").await;
    repo.run(&["checkout", "-q", "--detach", &sha]).await;

    let head = refs::head_info(repo.path()).await.expect("head_info");
    assert!(head.detached);
    assert_eq!(head.branch, None);
    assert_eq!(head.sha, sha);
}
