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
