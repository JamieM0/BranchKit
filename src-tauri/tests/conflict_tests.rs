//! Real-git fixtures for conflict state detection + Keep Panel region computation —
//! ARCHITECTURE.md §7.4/§7.5. Every fixture actually produces a conflict via `git merge`/
//! `git cherry-pick`, then exercises `conflict::detect_conflict_state` and
//! `conflict::conflict_regions` against the live, unmerged index — never hand-rolled marker text.

mod common;

use branchkit_lib::git::conflict::{self, ConflictKind, FileRegion};
use common::TestRepo;

fn text_of(lines: &[String]) -> String {
    lines.join("\n")
}

#[tokio::test]
async fn both_modified_conflict_is_detected_and_regions_computed() {
    let repo = TestRepo::init().await;
    repo.write("f.txt", "one\ntwo\nthree\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("f.txt", "one\nTWO-FEATURE\nthree\n");
    repo.commit_all("feature edits two").await;

    repo.checkout("main").await;
    repo.write("f.txt", "one\nTWO-MAIN\nthree\n");
    repo.commit_all("main edits two").await;

    let merge_result = repo.merge("feature").await;
    assert!(merge_result.is_err(), "expected a merge conflict");

    let state = conflict::detect_conflict_state(repo.path())
        .await
        .expect("detect")
        .expect("a conflict should be active");
    assert_eq!(state.kind, ConflictKind::Merge);
    assert_eq!(state.target_label, "main");
    assert_eq!(state.source_label, "feature");
    assert_eq!(state.files, vec!["f.txt".to_string()]);

    let regions = conflict::conflict_regions(repo.path(), "f.txt").await;
    assert!(!regions.ours_deleted);
    assert!(!regions.theirs_deleted);
    // one context line, one conflict region, one context line
    assert_eq!(regions.regions.len(), 3);
    assert_eq!(regions.regions[0], FileRegion::Context { lines: vec!["one".to_string()] });
    match &regions.regions[1] {
        FileRegion::Conflict {
            ours_lines,
            theirs_lines,
            same_both_prefix,
            same_both_suffix,
            ..
        } => {
            assert_eq!(ours_lines, &vec!["TWO-MAIN".to_string()]);
            assert_eq!(theirs_lines, &vec!["TWO-FEATURE".to_string()]);
            assert!(same_both_prefix.is_empty());
            assert!(same_both_suffix.is_empty());
        }
        other => panic!("expected a conflict region, got {other:?}"),
    }
    assert_eq!(regions.regions[2], FileRegion::Context { lines: vec!["three".to_string()] });
}

#[tokio::test]
async fn modify_delete_ours_deletes_theirs_modifies() {
    let repo = TestRepo::init().await;
    repo.write("f.txt", "one\ntwo\nthree\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("f.txt", "one\nTWO-FEATURE\nthree\n");
    repo.commit_all("feature edits").await;

    repo.checkout("main").await;
    repo.remove("f.txt");
    repo.stage(&["f.txt"]).await;
    repo.commit("main deletes file").await;

    let merge_result = repo.merge("feature").await;
    assert!(merge_result.is_err(), "expected a modify/delete conflict");

    let state = conflict::detect_conflict_state(repo.path())
        .await
        .expect("detect")
        .expect("a conflict should be active");
    assert_eq!(state.kind, ConflictKind::Merge);
    assert_eq!(state.files, vec!["f.txt".to_string()]);

    let regions = conflict::conflict_regions(repo.path(), "f.txt").await;
    assert!(regions.ours_deleted, "main (ours) deleted the file");
    assert!(!regions.theirs_deleted);
    // Whole file collapses into one big conflict region: ours has nothing, theirs has its edit.
    assert_eq!(regions.regions.len(), 1);
    match &regions.regions[0] {
        FileRegion::Conflict { ours_lines, theirs_lines, .. } => {
            assert!(ours_lines.is_empty(), "ours deleted everything");
            assert_eq!(text_of(theirs_lines), "one\nTWO-FEATURE\nthree");
        }
        other => panic!("expected a conflict region, got {other:?}"),
    }
}

#[tokio::test]
async fn modify_delete_theirs_deletes_ours_modifies() {
    let repo = TestRepo::init().await;
    repo.write("f.txt", "one\ntwo\nthree\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.remove("f.txt");
    repo.stage(&["f.txt"]).await;
    repo.commit("feature deletes file").await;

    repo.checkout("main").await;
    repo.write("f.txt", "one\nTWO-MAIN\nthree\n");
    repo.commit_all("main edits").await;

    let merge_result = repo.merge("feature").await;
    assert!(merge_result.is_err(), "expected a modify/delete conflict");

    let regions = conflict::conflict_regions(repo.path(), "f.txt").await;
    assert!(!regions.ours_deleted);
    assert!(regions.theirs_deleted, "feature (theirs) deleted the file");
    assert_eq!(regions.regions.len(), 1);
    match &regions.regions[0] {
        FileRegion::Conflict { ours_lines, theirs_lines, .. } => {
            assert_eq!(text_of(ours_lines), "one\nTWO-MAIN\nthree");
            assert!(theirs_lines.is_empty(), "theirs deleted everything");
        }
        other => panic!("expected a conflict region, got {other:?}"),
    }
}

#[tokio::test]
async fn both_added_conflict_has_no_common_base() {
    let repo = TestRepo::init().await;
    repo.write("seed.txt", "seed\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("new.txt", "feature version\n");
    repo.commit_all("feature adds new.txt").await;

    repo.checkout("main").await;
    repo.write("new.txt", "main version\n");
    repo.commit_all("main adds new.txt").await;

    let merge_result = repo.merge("feature").await;
    assert!(merge_result.is_err(), "expected a both-added conflict");

    let regions = conflict::conflict_regions(repo.path(), "new.txt").await;
    assert!(!regions.ours_deleted);
    assert!(!regions.theirs_deleted);
    assert_eq!(regions.regions.len(), 1);
    match &regions.regions[0] {
        FileRegion::Conflict { ours_lines, theirs_lines, base_start, base_end, .. } => {
            assert_eq!(*base_start, 0);
            assert_eq!(*base_end, 0);
            assert_eq!(text_of(ours_lines), "main version");
            assert_eq!(text_of(theirs_lines), "feature version");
        }
        other => panic!("expected a conflict region, got {other:?}"),
    }
}

#[tokio::test]
async fn marker_like_content_in_real_files_is_not_specially_parsed() {
    let repo = TestRepo::init().await;
    let base_content = "<<<<<<< literal\n=======\n>>>>>>> literal\nkeep me\n";
    repo.write("f.txt", base_content);
    repo.commit_all("base with marker-like text").await;

    repo.checkout_new("feature").await;
    repo.write(
        "f.txt",
        "<<<<<<< literal\n=======\n>>>>>>> literal\nfeature change\n",
    );
    repo.commit_all("feature edits last line").await;

    repo.checkout("main").await;
    repo.write(
        "f.txt",
        "<<<<<<< literal\n=======\n>>>>>>> literal\nmain change\n",
    );
    repo.commit_all("main edits last line").await;

    let merge_result = repo.merge("feature").await;
    assert!(merge_result.is_err(), "expected a conflict");

    let regions = conflict::conflict_regions(repo.path(), "f.txt").await;
    // The three marker-like lines are untouched context; only the real edit is a conflict.
    let context_text: String = regions
        .regions
        .iter()
        .filter_map(|r| match r {
            FileRegion::Context { lines } => Some(lines.join("\n")),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(context_text.contains("<<<<<<< literal"));
    assert!(context_text.contains("======="));
    assert!(context_text.contains(">>>>>>> literal"));
    let conflict_region = regions
        .regions
        .iter()
        .find(|r| matches!(r, FileRegion::Conflict { .. }))
        .expect("expected exactly one conflict region for the real edit");
    match conflict_region {
        FileRegion::Conflict { ours_lines, theirs_lines, .. } => {
            assert_eq!(text_of(ours_lines), "main change");
            assert_eq!(text_of(theirs_lines), "feature change");
        }
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn three_conflicted_files_at_once_are_all_listed() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "a\n");
    repo.write("b.txt", "b\n");
    repo.write("c.txt", "c\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("a.txt", "a-feature\n");
    repo.write("b.txt", "b-feature\n");
    repo.write("c.txt", "c-feature\n");
    repo.commit_all("feature edits all three").await;

    repo.checkout("main").await;
    repo.write("a.txt", "a-main\n");
    repo.write("b.txt", "b-main\n");
    repo.write("c.txt", "c-main\n");
    repo.commit_all("main edits all three").await;

    let merge_result = repo.merge("feature").await;
    assert!(merge_result.is_err(), "expected a conflict in all three files");

    let state = conflict::detect_conflict_state(repo.path())
        .await
        .expect("detect")
        .expect("a conflict should be active");
    let mut files = state.files.clone();
    files.sort();
    assert_eq!(files, vec!["a.txt".to_string(), "b.txt".to_string(), "c.txt".to_string()]);

    for path in &files {
        let regions = conflict::conflict_regions(repo.path(), path).await;
        assert!(
            regions.regions.iter().any(|r| matches!(r, FileRegion::Conflict { .. })),
            "expected a conflict region in {path}"
        );
    }
}

#[tokio::test]
async fn continue_and_abort_work_for_a_merge_conflict() {
    let repo = TestRepo::init().await;
    repo.write("f.txt", "one\ntwo\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("f.txt", "one\nTWO-FEATURE\n");
    repo.commit_all("feature edits").await;

    repo.checkout("main").await;
    repo.write("f.txt", "one\nTWO-MAIN\n");
    let main_tip = repo.commit_all("main edits").await;

    // --- abort: repo returns exactly to main's pre-merge tip, no conflict remains ---
    assert!(repo.merge("feature").await.is_err());
    assert!(conflict::detect_conflict_state(repo.path()).await.unwrap().is_some());
    conflict::abort_conflict_impl(repo.path()).await.expect("abort should succeed");
    assert!(conflict::detect_conflict_state(repo.path()).await.unwrap().is_none());
    assert_eq!(repo.head_sha().await, main_tip);

    // --- continue: resolve by hand, stage, then continue commits the merge ---
    assert!(repo.merge("feature").await.is_err());
    repo.write("f.txt", "one\nTWO-RESOLVED\n");
    repo.stage(&["f.txt"]).await;
    conflict::continue_conflict_impl(repo.path()).await.expect("continue should succeed");
    assert!(conflict::detect_conflict_state(repo.path()).await.unwrap().is_none());
    let head = repo.run(&["rev-list", "--parents", "-n1", "HEAD"]).await;
    let parents = String::from_utf8_lossy(&head.stdout);
    assert_eq!(parents.split_whitespace().count(), 3, "continue should produce a merge commit");
    let content = std::fs::read_to_string(repo.path().join("f.txt")).unwrap();
    assert_eq!(content, "one\nTWO-RESOLVED\n");
}

#[tokio::test]
async fn continue_and_abort_work_for_a_cherry_pick_conflict() {
    let repo = TestRepo::init().await;
    repo.write("f.txt", "one\ntwo\n");
    repo.commit_all("base").await;

    repo.checkout_new("feature").await;
    repo.write("f.txt", "one\nTWO-FEATURE\n");
    let feature_commit = repo.commit_all("feature edits two").await;

    repo.checkout("main").await;
    repo.write("f.txt", "one\nTWO-MAIN\n");
    let main_tip = repo.commit_all("main edits two").await;

    // --- abort ---
    let pick = repo.try_run(&["cherry-pick", &feature_commit]).await;
    assert!(pick.is_err(), "expected a cherry-pick conflict");
    let state = conflict::detect_conflict_state(repo.path())
        .await
        .unwrap()
        .expect("a conflict should be active");
    assert_eq!(state.kind, ConflictKind::CherryPick);
    assert_eq!(state.source_label, "feature edits two");

    conflict::abort_conflict_impl(repo.path()).await.expect("abort should succeed");
    assert!(conflict::detect_conflict_state(repo.path()).await.unwrap().is_none());
    assert_eq!(repo.head_sha().await, main_tip);

    // --- continue ---
    let pick = repo.try_run(&["cherry-pick", &feature_commit]).await;
    assert!(pick.is_err());
    repo.write("f.txt", "one\nTWO-RESOLVED\n");
    repo.stage(&["f.txt"]).await;
    conflict::continue_conflict_impl(repo.path()).await.expect("continue should succeed");
    assert!(conflict::detect_conflict_state(repo.path()).await.unwrap().is_none());
    let subject = repo.run(&["log", "-1", "--format=%s"]).await;
    assert_eq!(
        String::from_utf8_lossy(&subject.stdout).trim(),
        "feature edits two"
    );
}
