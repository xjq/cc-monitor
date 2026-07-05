// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod config;
mod db;
mod models;
mod commands;
mod poller;

use std::time::Duration;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

struct DebounceTracker {
    overlay_handle: Option<tauri::async_runtime::JoinHandle<()>>,
    detail_handle: Option<tauri::async_runtime::JoinHandle<()>>,
}

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示悬浮窗", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏悬浮窗", true, None::<&str>)?;
    let relocate = MenuItem::with_id(app, "relocate", "重新定位 DB…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &relocate, &quit])?;
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
            .inner_size(260.0, 96.0)
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running cc-monitor");
}
