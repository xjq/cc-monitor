use tauri::{AppHandle, Emitter, Manager};

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
pub async fn pick_db_path(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    // Use a channel to convert callback to synchronous result
    let (tx, rx) = std::sync::mpsc::channel();

    app.dialog()
        .file()
        .add_filter("SQLite DB", &["db"])
        .pick_file(move |path| {
            let picked = path.map(|f| f.into_path().ok()).flatten()
                .map(|p| p.to_string_lossy().to_string());
            let _ = tx.send(picked);
        });

    // Run the blocking recv on the blocking pool instead of a worker thread
    let result = tauri::async_runtime::spawn_blocking(move || {
        match rx.recv() {
            Ok(Some(p)) => {
                let mut cfg = config::load(&app);
                cfg.db_path = Some(p);
                config::save(&app, &cfg);
                true
            }
            Ok(None) => false,
            Err(_) => false,
        }
    }).await.map_err(|e| e.to_string())?;

    Ok(result)
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> serde_json::Value {
    let cfg = config::load(&app);
    serde_json::json!({
        "usd_to_cny": cfg.usd_to_cny,
        "poll_interval_sec": cfg.poll_interval_sec,
        "font_scale": config::normalize_scale(&cfg.font_scale),
    })
}

/// Set the overlay font scale (small/medium/large), persist it, resize the
/// overlay window, and broadcast the change to the frontend.
#[tauri::command]
pub fn set_font_scale(app: AppHandle, scale: String) -> Result<(), String> {
    let scale = config::normalize_scale(&scale).to_string();
    let mut cfg = config::load(&app);
    cfg.font_scale = scale.clone();
    config::save(&app, &cfg);

    if let Some(w) = app.get_webview_window("overlay") {
        let (ww, hh) = crate::overlay_size_for(&scale);
        let _ = w.set_size(tauri::LogicalSize::new(ww, hh));
    }
    let _ = app.emit("font-scale-changed", &scale);
    Ok(())
}
