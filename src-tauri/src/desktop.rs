//! Native desktop chrome and application commands (DESIGN_SPEC §§3, 10, 14).
use tauri::{menu::{AboutMetadata, Menu, MenuItem, SubmenuBuilder}, AppHandle, Emitter, Manager};

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open_repo", "Open Repository…", true, Some("CmdOrCtrl+O"))?;
    let clone = MenuItem::with_id(app, "clone_repo", "Clone Repository…", true, Some("CmdOrCtrl+Shift+O"))?;
    let picker = MenuItem::with_id(app, "repo_picker", "Open Repository Tab…", true, Some("CmdOrCtrl+T"))?;
    let close = MenuItem::with_id(app, "close_repo", "Close Repository Tab", false, Some("CmdOrCtrl+W"))?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let palette = MenuItem::with_id(app, "command_palette", "Command Palette…", true, Some("CmdOrCtrl+K"))?;
    let file = SubmenuBuilder::new(app, "File").item(&open).item(&clone).item(&picker).separator().item(&close);
    #[cfg(not(target_os = "macos"))]
    let file = file.separator().item(&settings).separator().quit();
    let file = file.build()?;
    let edit = SubmenuBuilder::new(app, "Edit").undo().redo().separator().cut().copy().paste().select_all().build()?;
    let view = SubmenuBuilder::new(app, "View").item(&palette).separator().fullscreen().build()?;
    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().build()?;
    let menu = Menu::new(app)?;
    #[cfg(target_os = "macos")]
    {
        let app_menu = SubmenuBuilder::new(app, "BranchKit")
            .about(Some(AboutMetadata { name: Some("BranchKit".into()), version: Some(app.package_info().version.to_string()), ..Default::default() }))
            .separator().item(&settings).separator().services().separator()
            .hide().hide_others().show_all().separator().quit().build()?;
        menu.append(&app_menu)?;
    }
    menu.append_items(&[&file, &edit, &view, &window])?;
    app.set_menu(menu)?;
    Ok(())
}

pub fn handle_menu(app: &AppHandle, id: &str) {
    if !matches!(id, "open_repo" | "clone_repo" | "repo_picker" | "close_repo" | "settings" | "command_palette") { return; }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("app://menu", id);
    }
}

#[tauri::command]
pub fn set_window_context(app: AppHandle, title: String, can_close_tab: bool, modal_open: bool) -> Result<(), crate::error::AppError> {
    if let Some(window) = app.get_webview_window("main") { window.set_title(&title)?; }
    if let Some(menu) = app.menu() {
        // Menu::get searches direct children. Commands live one level down in native submenus.
        for submenu in menu.items()? {
            let Some(submenu) = submenu.as_submenu() else { continue; };
            for id in ["open_repo", "clone_repo", "repo_picker", "close_repo", "settings", "command_palette"] {
                if let Some(item) = submenu.get(id) {
                    if let Some(item) = item.as_menuitem() {
                        item.set_enabled(!modal_open && (id != "close_repo" || can_close_tab))?;
                    }
                }
            }
        }
    }
    Ok(())
}
