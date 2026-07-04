mod common;

use branchkit_lib::git::status::{self, FileStatusCode, StatusEntryKind};
use common::TestRepo;

#[tokio::test]
async fn reports_untracked_file() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;
    repo.write("new.txt", "new\n");

    let report = status::status(repo.path()).await.expect("status");
    let entry = report.entries.iter().find(|e| e.path == "new.txt").unwrap();
    assert_eq!(entry.kind, StatusEntryKind::Untracked);
    assert_eq!(entry.index_status, FileStatusCode::Untracked);
    assert_eq!(entry.worktree_status, FileStatusCode::Untracked);
}

#[tokio::test]
async fn reports_partially_staged_file_in_both_sections() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    repo.write("a.txt", "two\n");
    repo.stage(&["a.txt"]).await;
    // Further unstaged edit on top of the staged change.
    repo.write("a.txt", "three\n");

    let report = status::status(repo.path()).await.expect("status");
    let entry = report.entries.iter().find(|e| e.path == "a.txt").unwrap();
    assert_eq!(entry.index_status, FileStatusCode::Modified);
    assert_eq!(entry.worktree_status, FileStatusCode::Modified);
}

#[tokio::test]
async fn reports_staged_rename_with_orig_path() {
    let repo = TestRepo::init().await;
    repo.write("old_name.txt", "some fairly unique content to survive similarity check\n");
    repo.commit_all("base").await;

    repo.remove("old_name.txt");
    repo.write("new_name.txt", "some fairly unique content to survive similarity check\n");
    repo.stage_all().await;

    let report = status::status(repo.path()).await.expect("status");
    let entry = report
        .entries
        .iter()
        .find(|e| e.path == "new_name.txt")
        .expect("renamed entry present");
    assert_eq!(entry.kind, StatusEntryKind::RenamedOrCopied);
    assert_eq!(entry.orig_path.as_deref(), Some("old_name.txt"));
    assert_eq!(entry.index_status, FileStatusCode::Renamed);
}

#[tokio::test]
async fn reports_unmerged_conflict_with_marker_like_content() {
    let repo = TestRepo::init().await;
    repo.write("file.txt", "line1\nline2\nline3\n");
    repo.commit_all("base").await;

    repo.checkout_new("theirs").await;
    // Content that itself contains marker-like text — must not confuse status parsing, which
    // only reads the porcelain header, never the file body.
    repo.write("file.txt", "line1\n<<<<<<< not a real marker\nline3\n");
    repo.commit_all("theirs change").await;

    repo.checkout("main").await;
    repo.write("file.txt", "line1\nline2 changed\nline3\n");
    repo.commit_all("ours change").await;

    let merge_result = repo.merge("theirs").await;
    assert!(merge_result.is_err(), "expected a conflicting merge");

    let report = status::status(repo.path()).await.expect("status");
    let entry = report
        .entries
        .iter()
        .find(|e| e.path == "file.txt")
        .expect("conflicted entry present");
    assert_eq!(entry.kind, StatusEntryKind::Unmerged);
    assert_eq!(entry.index_status, FileStatusCode::UpdatedButUnmerged);
    assert_eq!(entry.worktree_status, FileStatusCode::UpdatedButUnmerged);
}

#[tokio::test]
async fn branch_header_reflects_current_branch() {
    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    let report = status::status(repo.path()).await.expect("status");
    assert_eq!(report.branch.head.as_deref(), Some("main"));
}

// Only Linux filesystems accept arbitrary non-UTF8 byte sequences in filenames — macOS's
// filesystems (APFS/HFS+) enforce valid UTF-8 at the OS level, and Windows paths are UTF-16, so
// this fixture (ARCHITECTURE.md §12) only applies there.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn handles_non_utf8_filename() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let repo = TestRepo::init().await;
    repo.write("a.txt", "one\n");
    repo.commit_all("base").await;

    // 0xFF is not valid UTF-8 on its own.
    let raw_name = OsStr::from_bytes(b"bad-\xffname.txt");
    std::fs::write(repo.path().join(raw_name), b"content").expect("write non-utf8 filename");

    let report = status::status(repo.path()).await.expect("status");
    // The parser must not panic or drop the entry; from_utf8_lossy replaces the invalid byte.
    assert!(report
        .entries
        .iter()
        .any(|e| e.path.starts_with("bad-") && e.path.contains("name.txt")));
}
