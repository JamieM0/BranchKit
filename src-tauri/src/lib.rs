pub mod error;
pub mod events;
pub mod git;
pub mod repo;
pub mod state;
pub mod watcher;

use state::AppState;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
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
        .invoke_handler(tauri::generate_handler![
            greet,
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
            git::ops::pull,
            git::ops::push,
            git::ops::set_upstream,
            git::ops::branch_divergence,
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
            git::blob::get_blob,
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
