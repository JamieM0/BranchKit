pub mod error;
pub mod events;
pub mod git;
pub mod repo;
pub mod state;
pub mod watcher;

use tauri::Manager;

use state::AppState;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Writes `contents` to `path` on disk — the "Create patch from commit/file" menu items save the
/// patch text the frontend already fetched via IPC to wherever the native save dialog picked.
/// Plain `std::fs`, no new crate: adding `tauri-plugin-fs` for this one write isn't worth a new
/// dependency (CLAUDE.md's hard rule) when the frontend already has the save path from the
/// `tauri-plugin-dialog` save dialog and Rust can just write the file directly.
#[tauri::command]
fn save_text_file(path: String, contents: String) -> Result<(), error::AppError> {
    std::fs::write(path, contents)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            // Best-effort discard-trash purge (ARCHITECTURE.md §7.3) — never blocks startup.
            git::discard::purge_old_entries(&app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Drives the auto-fetch focus gate (ARCHITECTURE.md §7.2) and macOS app-nap pause
            // (§14) — auto-fetch simply checks this flag on its next tick rather than being
            // started/stopped per window.
            if let tauri::WindowEvent::Focused(focused) = event {
                window
                    .app_handle()
                    .state::<AppState>()
                    .focused
                    .store(*focused, std::sync::atomic::Ordering::SeqCst);
            }
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            save_text_file,
            repo::open_repo,
            repo::clone_repo,
            repo::close_repo,
            repo::list_recents,
            repo::check_git_identity,
            repo::set_git_identity,
            git::log::get_graph,
            git::log::get_commit_meta,
            git::refs::get_refs,
            git::worktree::get_worktrees,
            git::ops::checkout_branch,
            git::ops::checkout_remote,
            git::ops::checkout_previous,
            git::ops::checkout_detached,
            git::ops::create_branch,
            git::ops::rename_branch,
            git::ops::delete_branch,
            git::ops::recreate_branch,
            git::ops::merge_ref,
            git::ops::rebase_onto,
            git::ops::fast_forward,
            git::ops::set_upstream,
            git::ops::branch_divergence,
            git::ops::checkout_stash_and_switch,
            git::ops::cherry_pick,
            git::ops::revert_commit,
            git::ops::reset_to,
            git::ops::create_tag,
            git::ops::delete_tag,
            git::ops::get_remote_url,
            git::ops::ignore_path,
            git::stash::stash_push,
            git::stash::stash_pop,
            git::stash::stash_apply,
            git::stash::stash_drop,
            git::stash::get_stash_patch,
            git::remote::fetch_all,
            git::remote::pull,
            git::remote::push,
            git::remote::publish,
            git::status::get_status,
            git::stage::stage_file,
            git::stage::unstage_file,
            git::stage::stage_all,
            git::stage::unstage_all,
            git::stage::stage_lines,
            git::stage::unstage_lines,
            git::commit::commit,
            git::commit::undo_commit,
            git::discard::discard_file,
            git::discard::discard_hunk,
            git::discard::discard_all,
            git::discard::list_discarded,
            git::discard::restore_discarded,
            git::diff::get_diff_worktree,
            git::diff::get_diff_staged,
            git::diff::get_diff_commit,
            git::diff::get_diff_two_commits,
            git::diff::get_commit_files,
            git::diff::get_diff_files,
            git::diff::get_diff_commit_vs_working,
            git::diff::get_commit_files_vs_working,
            git::diff::create_patch_from_commit,
            git::diff::create_patch_from_file,
            git::blob::get_blob,
            git::conflict::get_conflict_state,
            git::conflict::continue_conflict,
            git::conflict::abort_conflict,
            git::conflict::get_conflict_regions,
            git::conflict::confirm_file,
            git::conflict::reopen_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_includes_name() {
        assert_eq!(
            greet("BranchKit"),
            "Hello, BranchKit! You've been greeted from Rust!"
        );
    }
}
