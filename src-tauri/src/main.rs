// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `credential.helper=!"<this binary>" credential-helper` (ARCHITECTURE.md §8) means git spawns
    // us as a plain subprocess speaking the get/store/erase protocol on stdin/stdout — intercepted
    // here, before Tauri touches a window (or anything else), and exits immediately rather than
    // falling through to the normal app.
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("credential-helper") {
        if let Some(op) = args.next() {
            branchkit_lib::credentials::run_credential_helper_cli(&op);
        }
        return;
    }

    branchkit_lib::run()
}
