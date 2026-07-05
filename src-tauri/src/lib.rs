// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod config;
mod db;
mod models;
mod commands;
mod poller;

use std::time::Duration;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

struct DebounceTracker {
    overlay_handle: Option<tauri::async_runtime::JoinHandle<()>>,
    detail_handle: Option<tauri::async_runtime::JoinHandle<()>>,
}

/// Overlay window/card size (logical px) for each font-scale level.
/// Keep in sync with the CSS `--w/--h` vars per `.size-*` class.
pub fn overlay_size_for(scale: &str) -> (f64, f64) {
    match config::normalize_scale(scale) {
        "small" => (256.0, 144.0),
        "large" => (384.0, 216.0),
        _ => (320.0, 180.0),
    }
}

/// Held in Tauri state so the tray handler can refresh check marks without
/// rebuilding the whole menu or looking up the tray icon.
struct FontMenuItems {
    small: CheckMenuItem<tauri::Wry>,
    medium: CheckMenuItem<tauri::Wry>,
    large: CheckMenuItem<tauri::Wry>,
}

impl FontMenuItems {
    fn set_active(&self, scale: &str) {
        let scale = config::normalize_scale(scale);
        let _ = self.small.set_checked(scale == "small");
        let _ = self.medium.set_checked(scale == "medium");
        let _ = self.large.set_checked(scale == "large");
    }
}

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let cfg = config::load(app);
    let scale = config::normalize_scale(&cfg.font_scale);

    let show = MenuItem::with_id(app, "show", "显示悬浮窗", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏悬浮窗", true, None::<&str>)?;
    let fs = CheckMenuItem::with_id(app, "font-small", "小", scale == "small", true, None::<&str>)?;
    let fm = CheckMenuItem::with_id(app, "font-medium", "中", scale == "medium", true, None::<&str>)?;
    let fl = CheckMenuItem::with_id(app, "font-large", "大", scale == "large", true, None::<&str>)?;
    let font_submenu = Submenu::with_items(app, "字号", true, &[&fs, &fm, &fl])?;
    let relocate = MenuItem::with_id(app, "relocate", "重新定位 DB…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &font_submenu, &relocate, &quit])?;

    app.manage(FontMenuItems { small: fs, medium: fm, large: fl });

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("cc-monitor")
        .icon(app.default_window_icon().expect("default window icon configured").clone())
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("overlay") {
                    let _ = w.show();
                }
            }
            "hide" => {
                if let Some(w) = app.get_webview_window("overlay") {
                    let _ = w.hide();
                }
            }
            "font-small" => apply_font_scale(app, "small"),
            "font-medium" => apply_font_scale(app, "medium"),
            "font-large" => apply_font_scale(app, "large"),
            "relocate" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = commands::pick_db_path(app);
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button, .. } = e {
                if button != MouseButton::Left {
                    return;
                }
            } else {
                return;
            }
            let app = tray.app_handle();
            if let Some(w) = app.get_webview_window("overlay") {
                if w.is_visible().unwrap_or(false) {
                    let _ = w.hide();
                } else {
                    let _ = w.show();
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Apply a new font scale: persist config, resize the overlay, notify the
/// frontend, and refresh the tray check marks.
fn apply_font_scale(app: &tauri::AppHandle, scale: &str) {
    let scale = config::normalize_scale(scale);
    let mut cfg = config::load(app);
    cfg.font_scale = scale.into();
    config::save(app, &cfg);

    if let Some(w) = app.get_webview_window("overlay") {
        let (ww, hh) = overlay_size_for(scale);
        let _ = w.set_size(LogicalSize::new(ww, hh));
    }
    let _ = app.emit("font-scale-changed", scale);

    if let Some(items) = app.try_state::<FontMenuItems>() {
        items.set_active(scale);
    }
}

fn debounce_save_position(app: tauri::AppHandle, label: &str, x: i32, y: i32, tracker: Arc<Mutex<DebounceTracker>>) {
    let app = app.clone();
    let label = label.to_string();
    let mut guard = tracker.lock().unwrap();
    let handle_to_abort = if label == "overlay" {
        guard.overlay_handle.take()
    } else if label == "detail" {
        guard.detail_handle.take()
    } else {
        None
    };

    // Abort previous pending task if exists
    if let Some(prev) = handle_to_abort {
        prev.abort();
    }

    let label_for_async = label.clone();
    let new_handle = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if label_for_async == "overlay" {
            commands::save_overlay_position(app, x, y);
        } else if label_for_async == "detail" {
            commands::save_detail_position(app, x, y);
        }
    });

    if label == "overlay" {
        guard.overlay_handle = Some(new_handle);
    } else if label == "detail" {
        guard.detail_handle = Some(new_handle);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let cfg = config::load(app.handle());

            let overlay = WebviewWindowBuilder::new(
                app,
                "overlay",
                WebviewUrl::App("overlay.html".into()),
            )
            .title("cc-monitor")
            .inner_size(overlay_size_for(&cfg.font_scale).0, overlay_size_for(&cfg.font_scale).1)
            .position(cfg.overlay.x as f64, cfg.overlay.y as f64)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(cfg.overlay.visible)
            .build()?;

            let _detail = WebviewWindowBuilder::new(
                app,
                "detail",
                WebviewUrl::App("detail.html".into()),
            )
            .title("cc-monitor 详情")
            .inner_size(560.0, 440.0)
            .position(cfg.detail.x as f64, cfg.detail.y as f64)
            .visible(false)
            .build()?;

            // Create debounce tracker for position saves
            let debounce_tracker = Arc::new(Mutex::new(DebounceTracker {
                overlay_handle: None,
                detail_handle: None,
            }));

            // Persist window position on move (debounced).
            let app_handle = app.handle().clone();
            let tracker1 = debounce_tracker.clone();
            overlay.on_window_event(move |e| {
                if let WindowEvent::Moved(pos) = e {
                    debounce_save_position(app_handle.clone(), "overlay", pos.x, pos.y, tracker1.clone());
                }
            });
            let app_handle2 = app.handle().clone();
            let tracker2 = debounce_tracker.clone();
            if let Some(detail_win) = app.get_webview_window("detail") {
                detail_win.on_window_event(move |e| {
                    if let WindowEvent::Moved(pos) = e {
                        debounce_save_position(app_handle2.clone(), "detail", pos.x, pos.y, tracker2.clone());
                    }
                });
            }

            build_tray(app.handle())?;
            poller::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_today_detail,
            commands::save_overlay_position,
            commands::save_detail_position,
            commands::relocate_db,
            commands::show_detail_window,
            commands::pick_db_path,
            commands::get_settings,
            commands::set_font_scale,
        ])
        .run(tauri::generate_context!())
        .expect("error while running cc-monitor");
}
