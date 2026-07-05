use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::{config, db};

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let cfg = config::load(&app);
            let interval = Duration::from_secs(cfg.poll_interval_sec.max(1));
            tokio::time::sleep(interval).await;
            match config::resolve_db_path(&app) {
                None => {
                    let _ = app.emit("db-status", serde_json::json!({
                        "ok": false, "message": "cc-switch.db 未找到"
                    }));
                }
                Some(p) => match db::open_readonly(&p) {
                    Err(e) => {
                        let _ = app.emit("db-status", serde_json::json!({
                            "ok": false, "message": format!("读取失败: {e}")
                        }));
                    }
                    Ok(conn) => {
                        let since = db::midnight_unix_live();
                        match db::fetch_summary(&conn, since) {
                            Ok(s) => {
                                let _ = app.emit("db-status", serde_json::json!({ "ok": true, "message": "" }));
                                let _ = app.emit("usage-update", &s);
                            }
                            Err(e) => {
                                let _ = app.emit("db-status", serde_json::json!({
                                    "ok": false, "message": format!("查询失败: {e}")
                                }));
                            }
                        }
                    }
                },
            }
        }
    });
}
