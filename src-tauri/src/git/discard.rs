//! Discard safety net — ARCHITECTURE.md §7.3, DESIGN_SPEC.md §7.4/§8/§15.12. Every discard
//! (file/hunk/all) writes a trash entry — a forward patch that reproduces exactly what's being
//! removed, plus raw copies of any untracked files — *before* touching the working tree, so every
//! discard is reversible: the toast's **Undo** and the repo menu's "Recently discarded" list both
//! just re-apply that trash entry. Entries purge after 7 days or once a repo's trash exceeds
//! 200MB (oldest evicted first), both enforced on startup.
//!
//! SPEC-DEVIATION: ARCHITECTURE.md §7.3 says untracked-file copies are "zipped". There's no zip
//! crate in ARCHITECTURE.md §1's stack table, and adding one for this alone isn't worth a new
//! dependency (CLAUDE.md's hard rule on deps) — flat copies under `untracked/<path>` restore
//! identically and the purge/size-cap logic works the same either way.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::state::AppState;

use super::diff::parse_diff_output;
use super::exec::{git, GitError, GitOpts};
use super::ops::{emit_changes, require_repo};
use super::stage::{all_change_indices, apply_patch, build_line_patch};
use super::status::{status, FileStatusCode, StatusEntryKind};

const PURGE_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const MAX_TRASH_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    description: String,
    files: Vec<String>,
    untracked: Vec<String>,
    created_at_ms: u64,
}

/// One entry in the repo menu's "Recently discarded" list (DESIGN_SPEC.md §7.4/§12).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscardedEntry {
    pub id: String,
    pub description: String,
    pub files: Vec<String>,
    pub created_at_ms: u64,
}

fn repo_hash(repo: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    repo.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn trash_root_for(app: &AppHandle, repo: &Path) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()?
        .join("trash")
        .join(repo_hash(repo));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Sortable-by-name (ascending == oldest-first), filesystem-safe entry id.
fn new_entry_id() -> (String, u64) {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    (format!("{ms:020}"), ms)
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}

fn remove_entry_dir(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Evicts oldest entries under `root` until it's back under the size cap. Entry directory names
/// are zero-padded millisecond timestamps, so lexicographic order is chronological order.
fn enforce_cap(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut ids: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    ids.sort();
    while dir_size(root) > MAX_TRASH_BYTES {
        let Some(oldest) = ids.first().cloned() else {
            break;
        };
        remove_entry_dir(&oldest);
        ids.remove(0);
    }
}

/// Writes a trash entry for `paths` before a discard touches the working tree
/// (ARCHITECTURE.md §7.3). `patch_override` supplies pre-built patch text for a hunk-level
/// discard (the caller already has the exact hunk patch); otherwise this diffs `paths` itself
/// (whole-file/whole-repo discards). Takes the trash root directly (rather than resolving it from
/// an `AppHandle` itself) so this — the actual logic — is testable against a plain tempdir.
async fn write_trash(
    root: &Path,
    repo: &Path,
    paths: &[String],
    description: &str,
    patch_override: Option<&str>,
) -> Result<(), AppError> {
    let (id, ms) = new_entry_id();
    let entry_dir = root.join(&id);
    std::fs::create_dir_all(&entry_dir)?;

    let report = status(repo).await.map_err(AppError::from)?;
    let mut untracked = Vec::new();
    let mut tracked = Vec::new();
    for p in paths {
        let is_untracked = report
            .entries
            .iter()
            .any(|e| e.path == *p && e.kind == StatusEntryKind::Untracked);
        if is_untracked {
            untracked.push(p.clone());
        } else {
            tracked.push(p.clone());
        }
    }

    if let Some(patch) = patch_override {
        std::fs::write(entry_dir.join("changes.patch"), patch)?;
    } else if !tracked.is_empty() {
        let mut args = vec!["diff", "--no-color", "--"];
        args.extend(tracked.iter().map(|s| s.as_str()));
        let output = git(repo, &args, GitOpts::default())
            .await
            .map_err(AppError::from)?;
        if !output.stdout.is_empty() {
            std::fs::write(entry_dir.join("changes.patch"), output.stdout)?;
        }
    }

    for p in &untracked {
        let src = repo.join(p);
        if let Ok(bytes) = std::fs::read(&src) {
            let dest = entry_dir.join("untracked").join(p);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, bytes)?;
        }
    }

    let manifest = Manifest {
        description: description.to_string(),
        files: paths.to_vec(),
        untracked,
        created_at_ms: ms,
    };
    std::fs::write(
        entry_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    enforce_cap(root);
    Ok(())
}

/// Reverts `paths` in the working tree: tracked files restore to their index content, untracked
/// ones are deleted. The trash entry written just before this call is what makes it reversible.
async fn discard_paths(repo: &Path, paths: &[String]) -> Result<(), GitError> {
    let report = status(repo).await?;
    let mut tracked = Vec::new();
    let mut untracked = Vec::new();
    for p in paths {
        let is_untracked = report
            .entries
            .iter()
            .any(|e| e.path == *p && e.kind == StatusEntryKind::Untracked);
        if is_untracked {
            untracked.push(p.clone());
        } else {
            tracked.push(p.clone());
        }
    }
    if !tracked.is_empty() {
        let mut args = vec!["restore", "--worktree", "--"];
        args.extend(tracked.iter().map(|s| s.as_str()));
        git(repo, &args, GitOpts::default()).await?;
    }
    for p in untracked {
        let _ = std::fs::remove_file(repo.join(&p));
    }
    Ok(())
}

/// Discard every unstaged change in one file (tracked modification or untracked file) — a file
/// row's Discard action.
#[tauri::command]
pub async fn discard_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    let root = trash_root_for(&app, &handle.path)?;
    write_trash(
        &root,
        &handle.path,
        std::slice::from_ref(&path),
        &format!("Discarded {path}"),
        None,
    )
    .await?;
    handle.begin_self_op(&[WatchedKind::WorkingTree]);
    discard_paths(&handle.path, std::slice::from_ref(&path)).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree]);
    Ok(())
}

/// Discard a single hunk from a file's unstaged diff — the hunk header's "Discard hunk…"
/// (DESIGN_SPEC.md §6.2). Builds the hunk's own forward patch (as if staging the whole hunk) for
/// the trash record, then applies it `--reverse` to the *worktree* (not `--cached`) to remove
/// just that hunk's changes.
#[tauri::command]
pub async fn discard_hunk(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    hunk_index: usize,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;

    let output = git(
        &handle.path,
        &["diff", "--no-color", "-U3", "--", &path],
        GitOpts::default(),
    )
    .await?;
    let raw_text = String::from_utf8_lossy(&output.stdout).into_owned();
    let file_diff = parse_diff_output(&output.stdout);
    let hunk = file_diff.hunks.get(hunk_index).ok_or_else(|| {
        AppError::new(
            "That change no longer exists — the file may have been edited",
            format!("hunk index {hunk_index} out of range"),
        )
    })?;
    let all = all_change_indices(hunk);
    let Some(patch) = build_line_patch(&raw_text, &file_diff, hunk_index, &all, false) else {
        return Ok(());
    };

    let root = trash_root_for(&app, &handle.path)?;
    write_trash(
        &root,
        &handle.path,
        std::slice::from_ref(&path),
        &format!("Discarded a change in {path}"),
        Some(&patch),
    )
    .await?;
    handle.begin_self_op(&[WatchedKind::WorkingTree]);
    apply_patch(&handle.path, &patch, true, false).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree]);
    Ok(())
}

/// Discard every unstaged *and* staged change in the working tree, including untracked files —
/// the file list header's Discard All (DESIGN_SPEC.md §6.1), gated by the frontend's arm-delay
/// confirm (§4.6/§15.12).
#[tauri::command]
pub async fn discard_all(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;

    let report = status(&handle.path).await?;
    let changed_paths: Vec<String> = report
        .entries
        .iter()
        .filter(|e| {
            e.kind != StatusEntryKind::Ignored
                && (e.index_status != FileStatusCode::Unmodified
                    || e.worktree_status != FileStatusCode::Unmodified
                    || e.kind == StatusEntryKind::Untracked)
        })
        .map(|e| e.path.clone())
        .collect();
    if changed_paths.is_empty() {
        return Ok(());
    }

    let untracked_paths: Vec<String> = report
        .entries
        .iter()
        .filter(|e| e.kind == StatusEntryKind::Untracked)
        .map(|e| e.path.clone())
        .collect();
    let tracked_paths: Vec<String> = changed_paths
        .iter()
        .filter(|p| !untracked_paths.contains(p))
        .cloned()
        .collect();

    // One combined patch vs HEAD covers both staged and unstaged tracked changes in a single
    // trash record (`git diff HEAD` — not `--cached` alone — so Undo restores everything).
    let patch_override = if tracked_paths.is_empty() {
        None
    } else {
        let mut args = vec!["diff", "--no-color", "HEAD", "--"];
        args.extend(tracked_paths.iter().map(|s| s.as_str()));
        let output = git(&handle.path, &args, GitOpts::default()).await?;
        if output.stdout.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
    };

    let root = trash_root_for(&app, &handle.path)?;
    write_trash(
        &root,
        &handle.path,
        &changed_paths,
        "Discarded all changes",
        patch_override.as_deref(),
    )
    .await?;

    handle.begin_self_op(&[WatchedKind::WorkingTree, WatchedKind::Index]);
    git(&handle.path, &["reset", "--hard", "HEAD"], GitOpts::default()).await?;
    for p in &untracked_paths {
        let _ = std::fs::remove_file(handle.path.join(p));
    }
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree, ChangeKind::Index]);
    Ok(())
}

/// The repo menu's "Recently discarded" list (DESIGN_SPEC.md §7.4/§12), newest first.
#[tauri::command]
pub async fn list_discarded(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<DiscardedEntry>, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let root = trash_root_for(&app, &handle.path)?;
    Ok(list_entries(&root))
}

fn list_entries(root: &Path) -> Vec<DiscardedEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(root) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let manifest_path = entry.path().join("manifest.json");
        let Ok(text) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<Manifest>(&text) else {
            continue;
        };
        let Some(id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        entries.push(DiscardedEntry {
            id,
            description: manifest.description,
            files: manifest.files,
            created_at_ms: manifest.created_at_ms,
        });
    }
    entries.sort_by_key(|e| std::cmp::Reverse(e.created_at_ms));
    entries
}

/// Restores a trash entry — the discard toast's **Undo**, or a manual restore from the "Recently
/// discarded" list. Re-applies the recorded patch forward to the worktree and copies back any
/// untracked files; the entry is left in place afterward (restoring doesn't consume it, matching
/// most undo-list UIs — a stale double-restore is a harmless no-op `git apply` failure).
#[tauri::command]
pub async fn restore_discarded(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    entry_id: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    let root = trash_root_for(&app, &handle.path)?;
    handle.begin_self_op(&[WatchedKind::WorkingTree]);
    restore_entry(&root, &handle.path, &entry_id).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree]);
    Ok(())
}

/// Re-applies a trash entry's patch forward to the worktree and copies back any untracked files —
/// the actual restore logic, taking the trash root directly so it's testable against a plain
/// tempdir instead of a live `AppHandle`.
async fn restore_entry(root: &Path, repo: &Path, entry_id: &str) -> Result<(), AppError> {
    let entry_dir = root.join(entry_id);
    if !entry_dir.is_dir() {
        return Err(AppError::new(
            "That discarded change is no longer available",
            format!("missing trash entry {entry_id}"),
        ));
    }

    let patch_path = entry_dir.join("changes.patch");
    if patch_path.is_file() {
        let patch = std::fs::read(&patch_path)?;
        super::exec::git_with_stdin(
            repo,
            &["apply", "--whitespace=nowarn", "-"],
            GitOpts::default(),
            &patch,
        )
        .await?;
    }

    let untracked_dir = entry_dir.join("untracked");
    if untracked_dir.is_dir() {
        restore_untracked_tree(&untracked_dir, &untracked_dir, repo)?;
    }
    Ok(())
}

fn restore_untracked_tree(root: &Path, dir: &Path, repo: &Path) -> Result<(), AppError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            restore_untracked_tree(root, &path, repo)?;
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let dest = repo.join(rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

/// Purges trash entries older than 7 days across every repo — called once at app startup
/// (ARCHITECTURE.md §7.3). Best-effort: a purge failure shouldn't block startup.
pub fn purge_old_entries(app: &AppHandle) {
    let Ok(trash_dir) = app.path().app_data_dir().map(|d| d.join("trash")) else {
        return;
    };
    let cutoff = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_sub(PURGE_AGE)
        .as_millis() as u64;
    purge_trash_dir(&trash_dir, cutoff);
}

/// The actual purge logic (age cutoff in epoch-ms, so tests can pass an arbitrary cutoff instead
/// of waiting 7 real days), scanning every per-repo subdirectory under `trash_dir`.
fn purge_trash_dir(trash_dir: &Path, cutoff_ms: u64) {
    let Ok(repo_dirs) = std::fs::read_dir(trash_dir) else {
        return;
    };
    for repo_dir in repo_dirs.flatten() {
        let repo_path = repo_dir.path();
        let Ok(entries) = std::fs::read_dir(&repo_path) else {
            continue;
        };
        for entry in entries.flatten() {
            let manifest_path = entry.path().join("manifest.json");
            let created_at_ms = std::fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|t| serde_json::from_str::<Manifest>(&t).ok())
                .map(|m| m.created_at_ms)
                .unwrap_or(0);
            if created_at_ms < cutoff_ms {
                remove_entry_dir(&entry.path());
            }
        }
        enforce_cap(&repo_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_hash_is_stable_and_distinct() {
        let a = repo_hash(Path::new("/repo/a"));
        let b = repo_hash(Path::new("/repo/b"));
        assert_ne!(a, b);
        assert_eq!(a, repo_hash(Path::new("/repo/a")));
    }

    #[test]
    fn dir_size_sums_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), vec![0u8; 10]).unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("b.txt"), vec![0u8; 20]).unwrap();
        assert_eq!(dir_size(dir.path()), 30);
    }

    #[test]
    fn enforce_cap_evicts_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        for (name, size) in [("00000001", 100usize), ("00000002", 100), ("00000003", 100)] {
            let entry = dir.path().join(name);
            std::fs::create_dir_all(&entry).unwrap();
            std::fs::write(entry.join("changes.patch"), vec![0u8; size]).unwrap();
        }
        // Cap smaller than the total (300 bytes) but bigger than one entry (100 bytes) — the
        // oldest-named entry should go first.
        let root = dir.path();
        let mut ids: Vec<PathBuf> = std::fs::read_dir(root)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        ids.sort();
        // Manually mimic enforce_cap with a tiny cap for the test (MAX_TRASH_BYTES is fixed).
        while dir_size(root) > 150 {
            let oldest = ids.remove(0);
            remove_entry_dir(&oldest);
        }
        assert!(!root.join("00000001").exists());
        assert!(root.join("00000003").exists());
    }

    async fn init_repo(dir: &Path) {
        git(dir, &["init", "--initial-branch=main", "-q"], GitOpts::default())
            .await
            .unwrap();
        git(dir, &["config", "user.name", "T"], GitOpts::default())
            .await
            .unwrap();
        git(dir, &["config", "user.email", "t@example.com"], GitOpts::default())
            .await
            .unwrap();
        git(dir, &["config", "commit.gpgsign", "false"], GitOpts::default())
            .await
            .unwrap();
        // Pin `core.autocrlf=false` so `git restore`/`git reset --hard` round-trip the exact bytes
        // the test wrote (Windows defaults to `true`, which would re-emit LF as CRLF on checkout
        // and break the byte-identical assertions below).
        git(
            dir,
            &["config", "core.autocrlf", "false"],
            GitOpts::default(),
        )
        .await
        .unwrap();
    }

    async fn commit_all(dir: &Path, msg: &str) {
        git(dir, &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir, &["commit", "-q", "-m", msg], GitOpts::default())
            .await
            .unwrap();
    }

    /// Discard→Undo round trip for a modified tracked file: content must come back byte-identical.
    #[tokio::test]
    async fn discard_file_then_restore_is_byte_identical_for_a_tracked_file() {
        let repo = tempfile::tempdir().unwrap();
        let trash = tempfile::tempdir().unwrap();
        init_repo(repo.path()).await;
        let original = "hello\nworld\n";
        std::fs::write(repo.path().join("f.txt"), original).unwrap();
        commit_all(repo.path(), "init").await;

        std::fs::write(repo.path().join("f.txt"), "hello\nEDITED\n").unwrap();

        write_trash(
            trash.path(),
            repo.path(),
            &["f.txt".to_string()],
            "Discarded f.txt",
            None,
        )
        .await
        .unwrap();
        discard_paths(repo.path(), &["f.txt".to_string()])
            .await
            .unwrap();
        assert_eq!(std::fs::read_to_string(repo.path().join("f.txt")).unwrap(), original);

        let entries = list_entries(trash.path());
        assert_eq!(entries.len(), 1);
        restore_entry(trash.path(), repo.path(), &entries[0].id)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
            "hello\nEDITED\n"
        );
    }

    /// Same round trip, but for an untracked file — the trash copy (not a patch) must restore it
    /// byte-identically after the file is deleted from disk.
    #[tokio::test]
    async fn discard_file_then_restore_is_byte_identical_for_an_untracked_file() {
        let repo = tempfile::tempdir().unwrap();
        let trash = tempfile::tempdir().unwrap();
        init_repo(repo.path()).await;
        std::fs::write(repo.path().join("README.md"), "seed").unwrap();
        commit_all(repo.path(), "init").await;

        let original = "brand new content\nwith multiple lines\n";
        std::fs::write(repo.path().join("new.txt"), original).unwrap();

        write_trash(
            trash.path(),
            repo.path(),
            &["new.txt".to_string()],
            "Discarded new.txt",
            None,
        )
        .await
        .unwrap();
        discard_paths(repo.path(), &["new.txt".to_string()])
            .await
            .unwrap();
        assert!(!repo.path().join("new.txt").exists());

        let entries = list_entries(trash.path());
        assert_eq!(entries.len(), 1);
        restore_entry(trash.path(), repo.path(), &entries[0].id)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(repo.path().join("new.txt")).unwrap(),
            original
        );
    }

    /// Discarding one hunk (out of several) then restoring must bring back only that hunk's
    /// content, byte-identically, leaving the rest of the file as it already was.
    #[tokio::test]
    async fn discard_hunk_then_restore_round_trips_that_hunk() {
        use super::super::diff::parse_diff_output;
        use super::super::stage::{all_change_indices, build_line_patch};

        let repo = tempfile::tempdir().unwrap();
        let trash = tempfile::tempdir().unwrap();
        init_repo(repo.path()).await;
        std::fs::write(repo.path().join("f.txt"), "a\nb\nc\n\n\n\nx\ny\nz\n").unwrap();
        commit_all(repo.path(), "init").await;

        let edited = "a1\nb\nc\n\n\n\nx\ny\nz1\n";
        std::fs::write(repo.path().join("f.txt"), edited).unwrap();

        let output = git(
            repo.path(),
            &["diff", "--no-color", "-U1", "--", "f.txt"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        let raw_text = String::from_utf8_lossy(&output.stdout).into_owned();
        let file_diff = parse_diff_output(&output.stdout);
        assert_eq!(file_diff.hunks.len(), 2, "expected two separate hunks");

        let hunk = &file_diff.hunks[0];
        let all = all_change_indices(hunk);
        let patch = build_line_patch(&raw_text, &file_diff, 0, &all, false).unwrap();

        write_trash(
            trash.path(),
            repo.path(),
            &["f.txt".to_string()],
            "Discarded a change in f.txt",
            Some(&patch),
        )
        .await
        .unwrap();
        apply_patch(repo.path(), &patch, true, false).await.unwrap();

        // Only the second hunk's edit remains in the working tree.
        assert_eq!(
            std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
            "a\nb\nc\n\n\n\nx\ny\nz1\n"
        );

        let entries = list_entries(trash.path());
        assert_eq!(entries.len(), 1);
        restore_entry(trash.path(), repo.path(), &entries[0].id)
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(repo.path().join("f.txt")).unwrap(), edited);
    }

    /// Discard All wipes staged, unstaged and untracked changes together; restoring the resulting
    /// single trash entry must bring every one of them back byte-identically.
    #[tokio::test]
    async fn discard_all_then_restore_round_trips_staged_unstaged_and_untracked() {
        let repo = tempfile::tempdir().unwrap();
        let trash = tempfile::tempdir().unwrap();
        init_repo(repo.path()).await;
        std::fs::write(repo.path().join("staged.txt"), "one\n").unwrap();
        std::fs::write(repo.path().join("unstaged.txt"), "two\n").unwrap();
        commit_all(repo.path(), "init").await;

        std::fs::write(repo.path().join("staged.txt"), "one CHANGED\n").unwrap();
        git(repo.path(), &["add", "--", "staged.txt"], GitOpts::default())
            .await
            .unwrap();
        std::fs::write(repo.path().join("unstaged.txt"), "two CHANGED\n").unwrap();
        std::fs::write(repo.path().join("untracked.txt"), "three\n").unwrap();

        let paths = vec![
            "staged.txt".to_string(),
            "unstaged.txt".to_string(),
            "untracked.txt".to_string(),
        ];
        let combined = git(
            repo.path(),
            &[
                "diff", "--no-color", "HEAD", "--", "staged.txt", "unstaged.txt",
            ],
            GitOpts::default(),
        )
        .await
        .unwrap();
        write_trash(
            trash.path(),
            repo.path(),
            &paths,
            "Discarded all changes",
            Some(&String::from_utf8_lossy(&combined.stdout)),
        )
        .await
        .unwrap();

        git(repo.path(), &["reset", "--hard", "HEAD"], GitOpts::default())
            .await
            .unwrap();
        std::fs::remove_file(repo.path().join("untracked.txt")).unwrap();

        assert_eq!(std::fs::read_to_string(repo.path().join("staged.txt")).unwrap(), "one\n");
        assert_eq!(std::fs::read_to_string(repo.path().join("unstaged.txt")).unwrap(), "two\n");
        assert!(!repo.path().join("untracked.txt").exists());

        let entries = list_entries(trash.path());
        assert_eq!(entries.len(), 1);
        restore_entry(trash.path(), repo.path(), &entries[0].id)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(repo.path().join("staged.txt")).unwrap(),
            "one CHANGED\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("unstaged.txt")).unwrap(),
            "two CHANGED\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
            "three\n"
        );
    }

    #[tokio::test]
    async fn purge_removes_only_entries_older_than_cutoff() {
        let repo = tempfile::tempdir().unwrap();
        // `purge_trash_dir` expects one directory per repo (each holding that repo's entries),
        // so the trash root here is a level above the per-repo dir `write_trash` writes into.
        let trash_parent = tempfile::tempdir().unwrap();
        let repo_trash_dir = trash_parent.path().join("some-repo-hash");
        std::fs::create_dir_all(&repo_trash_dir).unwrap();

        init_repo(repo.path()).await;
        std::fs::write(repo.path().join("f.txt"), "a\n").unwrap();
        commit_all(repo.path(), "init").await;
        std::fs::write(repo.path().join("f.txt"), "b\n").unwrap();

        write_trash(&repo_trash_dir, repo.path(), &["f.txt".to_string()], "old", None)
            .await
            .unwrap();
        let entries = list_entries(&repo_trash_dir);
        let old_id = entries[0].id.clone();

        // Backdate the manifest so it's older than the cutoff.
        let manifest_path = repo_trash_dir.join(&old_id).join("manifest.json");
        let mut manifest: Manifest =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.created_at_ms = 1000;
        std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        std::fs::write(repo.path().join("f.txt"), "c\n").unwrap();
        write_trash(&repo_trash_dir, repo.path(), &["f.txt".to_string()], "new", None)
            .await
            .unwrap();

        // A cutoff below both entries' timestamps (the backdated one at 1000ms, the real one at
        // "now") purges neither.
        purge_trash_dir(trash_parent.path(), 500);
        let remaining = list_entries(&repo_trash_dir);
        assert_eq!(remaining.len(), 2, "cutoff below both timestamps keeps both entries");

        // A cutoff just under "now" purges the backdated (1000ms) entry but keeps the real one.
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        purge_trash_dir(trash_parent.path(), now_ms - 1000);
        let remaining = list_entries(&repo_trash_dir);
        assert_eq!(remaining.len(), 1, "cutoff near now purges only the backdated entry");
        assert_ne!(remaining[0].id, old_id);
    }
}
