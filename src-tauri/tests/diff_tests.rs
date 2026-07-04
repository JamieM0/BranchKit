mod common;

use branchkit_lib::git::diff::{self, DiffLineKind};
use common::TestRepo;

#[tokio::test]
async fn worktree_diff_reports_context_add_and_del() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "line1\nline2\nline3\n");
    repo.commit_all("base").await;
    repo.write("a.txt", "line1\nline2 changed\nline3\n");

    let file_diff = diff::diff_worktree(repo.path(), "a.txt", false)
        .await
        .expect("diff");
    assert!(!file_diff.is_binary);
    assert_eq!(file_diff.hunks.len(), 1);
    let kinds: Vec<DiffLineKind> = file_diff.hunks[0].lines.iter().map(|l| l.kind).collect();
    assert!(kinds.contains(&DiffLineKind::Context));
    assert!(kinds.contains(&DiffLineKind::Add));
    assert!(kinds.contains(&DiffLineKind::Del));
}

#[tokio::test]
async fn staged_diff_only_shows_index_changes() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    repo.write("a.txt", "two\n");
    repo.stage(&["a.txt"]).await;
    // Unstaged edit that must NOT appear in the --cached diff.
    repo.write("a.txt", "three\n");

    let staged = diff::diff_staged(repo.path(), "a.txt", false)
        .await
        .expect("staged diff");
    let all_text: String = staged.hunks[0]
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all_text.contains("two"));
    assert!(!all_text.contains("three"));
}

#[tokio::test]
async fn commit_diff_shows_that_commits_changes() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;
    repo.write("a.txt", "two\n");
    let sha = repo.commit_all("second").await;

    let file_diff = diff::diff_commit(repo.path(), &sha, "a.txt", false)
        .await
        .expect("commit diff");
    assert_eq!(file_diff.hunks.len(), 1);
    assert!(file_diff.hunks[0].lines.iter().any(|l| l.text == "two"));
}

#[tokio::test]
async fn two_commit_diff_spans_multiple_commits() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    let sha_a = repo.commit_all("first").await;
    repo.write("a.txt", "two\n");
    repo.commit_all("second").await;
    repo.write("a.txt", "three\n");
    let sha_c = repo.commit_all("third").await;

    let file_diff = diff::diff_two_commits(repo.path(), &sha_a, &sha_c, "a.txt", false)
        .await
        .expect("two-commit diff");
    assert!(file_diff.hunks[0].lines.iter().any(|l| l.text == "three"));
    assert!(!file_diff.hunks[0].lines.iter().any(|l| l.text == "two"));
}

#[tokio::test]
async fn no_trailing_newline_marker_is_preserved() {
    let repo = TestRepo::init().await;
    repo.write_bytes("a.txt", b"one\ntwo");
    repo.commit_all("base").await;
    repo.write_bytes("a.txt", b"one\ntwo changed");

    let file_diff = diff::diff_worktree(repo.path(), "a.txt", false)
        .await
        .expect("diff");
    let del_line = file_diff.hunks[0]
        .lines
        .iter()
        .find(|l| l.kind == DiffLineKind::Del)
        .expect("del line");
    let add_line = file_diff.hunks[0]
        .lines
        .iter()
        .find(|l| l.kind == DiffLineKind::Add)
        .expect("add line");
    assert!(del_line.no_newline_at_eof);
    assert!(add_line.no_newline_at_eof);
}

#[tokio::test]
async fn marker_like_content_does_not_confuse_hunk_parsing() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "line1\nline2\nline3\n");
    repo.commit_all("base").await;
    // Content that looks like a conflict marker but is genuinely just modified text — the
    // three-way conflict view (Keep Panel) never parses markers, but the plain diff hunk parser
    // must still treat this as an ordinary line, not a false hunk boundary.
    repo.write("a.txt", "line1\n<<<<<<< HEAD\nline3\n");

    let file_diff = diff::diff_worktree(repo.path(), "a.txt", false)
        .await
        .expect("diff");
    assert_eq!(file_diff.hunks.len(), 1);
    assert!(file_diff.hunks[0]
        .lines
        .iter()
        .any(|l| l.kind == DiffLineKind::Add && l.text == "<<<<<<< HEAD"));
}

#[tokio::test]
async fn binary_file_reports_no_hunks() {
    let repo = TestRepo::init().await;
    // Null byte forces git to treat the file as binary.
    repo.write_bytes("image.bin", b"\x89PNG\x00\x01\x02\x03");
    repo.commit_all("base").await;
    repo.write_bytes("image.bin", b"\x89PNG\x00\x01\x02\x04");

    let file_diff = diff::diff_worktree(repo.path(), "image.bin", false)
        .await
        .expect("diff");
    assert!(file_diff.is_binary);
    assert!(file_diff.hunks.is_empty());
}

#[tokio::test]
async fn image_extension_is_flagged() {
    let repo = TestRepo::init().await;
    repo.write_bytes("photo.png", b"\x89PNG\x00binarydata");
    repo.commit_all("base").await;
    repo.write_bytes("photo.png", b"\x89PNG\x00binarydatavariant");

    let file_diff = diff::diff_worktree(repo.path(), "photo.png", false)
        .await
        .expect("diff");
    assert!(file_diff.is_image);
}

#[tokio::test]
async fn ignore_whitespace_flag_hides_whitespace_only_changes() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "line1\nline2\nline3\n");
    repo.commit_all("base").await;
    repo.write("a.txt", "line1\nline2   \nline3\n");

    let with_whitespace = diff::diff_worktree(repo.path(), "a.txt", false)
        .await
        .expect("diff");
    assert_eq!(with_whitespace.hunks.len(), 1);

    let ignoring_whitespace = diff::diff_worktree(repo.path(), "a.txt", true)
        .await
        .expect("diff");
    assert!(ignoring_whitespace.hunks.is_empty());
}
