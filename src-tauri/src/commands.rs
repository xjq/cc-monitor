use tauri::{AppHandle, Manager};

use crate::{config, db, models::TodayDetail};

#[tauri::command]
pub fn get_today_detail(app: AppHandle) -> Option<TodayDetail> {
    let path = config::resolve_db_path(&app)?;
    let conn = db::open_readonly(&path).ok()?;
    let since = db::midnight_unix_live();
    db::fetch_detail(&conn, since).ok()
}

#[tauri::command]
pub fn save_overlay_position(app: AppHandle, x: i32, y: i32) {
    let mut cfg = config::load(&app);
    cfg.overlay.x = x;
    cfg.overlay.y = y;
    config::save(&app, &cfg);
}

#[tauri::command]
pub fn save_detail_position(app: AppHandle, x: i32, y: i32) {
    let mut cfg = config::load(&app);
    cfg.detail.x = x;
    cfg.detail.y = y;
    config::save(&app, &cfg);
}

fn relocate_db_inner(app: &AppHandle, path: String) -> bool {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return false;
    }
    let mut cfg = config::load(app);
    cfg.db_path = Some(path);
    config::save(app, &cfg);
    true
}

#[tauri::command]
pub fn relocate_db(app: AppHandle, path: String) -> bool {
    relocate_db_inner(&app, path)
}

#[tauri::command]
pub async fn show_detail_window(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("detail") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Open a native file picker and persist the chosen DB path. Returns true on success.
#[tauri::command]
pub fn pick_db_path(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;
    let app_clone = app.clone();
    app.dialog()
        .file()
        .add_filter("SQLite DB", &["db"])
        .pick_file(move |path| {
            let result = path.map(|f| f.into_path().ok()).flatten()
                .map(|p| p.to_string_lossy().to_string());
            if let Some(p) = result {
                relocate_db_inner(&app_clone, p);
            }
        });
    Ok(true)
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> serde_json::Value {
    let cfg = config::load(&app);
    serde_json::json!({ "usd_to_cny": cfg.usd_to_cny, "poll_interval_sec": cfg.poll_interval_sec })
}
