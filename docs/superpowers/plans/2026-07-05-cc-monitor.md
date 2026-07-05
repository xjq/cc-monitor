# cc-monitor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows always-on-top Tauri overlay that reads cc-switch's local SQLite DB and shows today's cumulative token usage and cost, with an expandable detail panel and tray icon.

**Architecture:** Tauri 2.x app. Rust backend (`rusqlite`, read-only) polls `~/.cc-switch/cc-switch.db` every 3s and emits a `usage-update` Tauri event; a detail command returns hourly/per-model/per-provider breakdowns. Two webview windows: a frameless always-on-top overlay and a normal detail window. Frontend is Svelte 5 + Vite + TypeScript.

**Tech Stack:** Tauri 2.x, Rust (stable), `rusqlite` (bundled), `chrono`, `tokio` (Tauri runtime), Svelte 5, Vite, TypeScript, `uPlot`, Vitest.

## Global Constraints

- **Platform:** Windows 10+ only.
- **Rust toolchain:** stable via rustup (must be installed first — see Task 1).
- **Node:** already installed (v25.5.0); npm as package manager.
- **SQLite access:** open with `OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`, then `PRAGMA query_only=1`. Never write to cc-switch's DB.
- **"Today":** local-midnight → now, computed from `created_at` (unix seconds) using the local UTC offset.
- **Defaults:** `poll_interval_sec = 3`, `usd_to_cny = 7.2`, DB path fallback `%USERPROFILE%\.cc-switch\cc-switch.db`.
- **Config file:** `%APPDATA%\cc-monitor\config.json`.
- **Cost honesty:** `total_cost_usd` is summed as-is (TEXT → REAL). Rows with value `0` (unpriced models like `glm-5.2`) are reported via `unpriced_rows`; the UI shows a ⚠ hint, never fabricates cost.
- **Commit style:** conventional commits (`feat:`, `test:`, `chore:`, `docs:`).
- **Commit co-author trailer:** end every commit message with:
  ```
  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  ```

---

## File Structure

```
cc-monitor/
├── docs/superpowers/{specs,plans}/...        # already present
├── package.json                              # npm, tauri CLI
├── vite.config.ts                            # multi-page build (overlay.html, detail.html, index.html)
├── tsconfig.json
├── src/                                      # frontend
│   ├── index.html                            # redirects/unused (scaffolder default)
│   ├── overlay.html
│   ├── detail.html
│   ├── overlay/
│   │   └── Overlay.svelte
│   ├── detail/
│   │   └── Detail.svelte
│   ├── lib/
│   │   ├── format.ts                         # number formatting (tested)
│   │   ├── format.test.ts
│   │   └── events.ts                         # usage-update subscription helpers
│   └── main.ts
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/                                # from scaffolder
│   └── src/
│       ├── main.rs                           # app builder, windows, tray, setup
│       ├── config.rs                         # config load/save, db path resolution (tested)
│       ├── models.rs                         # UsageSummary, TodayDetail structs
│       ├── db.rs                             # open_readonly, midnight_unix, fetch_summary, fetch_detail (tested)
│       ├── poller.rs                         # tokio loop emitting usage-update
│       └── commands.rs                       # #[tauri::command]s
└── tests/                                    # (rust unit tests live in src alongside)
```

Responsibilities: each Rust module has one concern (config / db / poller / commands / models). Frontend `lib/` holds pure tested helpers; `overlay/` and `detail/` are window components.

---

## Task 1: Scaffold Tauri + Svelte + TypeScript project

**Files:**
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `src/index.html`, `src/main.ts`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/build.rs`, `src-tauri/icons/`

**Interfaces:**
- Consumes: nothing (greenfield).
- Produces: a runnable `npm run tauri dev` skeleton app with the default window, before any custom logic.

- [ ] **Step 1: Install Rust toolchain (only if missing)**

Run:
```bash
cargo --version 2>/dev/null || echo "Rust missing"
```
If "Rust missing", download and run `rustup-init.exe` from https://rustup.rs/ (default options), then reopen the shell and verify:
```bash
cargo --version
rustc --version
```
Expected: both print a stable version (e.g. `cargo 1.x`).

- [ ] **Step 2: Scaffold into a temp dir, then merge into the project**

The project dir already has `docs/` and `.git/`; scaffolding directly can conflict. Scaffold into a sibling temp dir and copy in.

Run (from `C:\Users\xjq\src`):
```bash
cd /c/Users/xjq/src
npm create tauri-app@latest cc-monitor-tmp -- --template svelte-ts --manager npm --identifier com.ccmonitor.desktop -y
cp -a cc-monitor-tmp/. cc-monitor/
rm -rf cc-monitor-tmp
cd cc-monitor
npm install
```
Expected: `cc-monitor/` now contains `package.json`, `src/`, `src-tauri/`, `vite.config.ts`, etc.

- [ ] **Step 3: Verify the dev build runs**

Run:
```bash
npm run tauri dev
```
Expected: Rust compiles (downloads crates first time), a window opens showing the Svelte welcome page, no errors. Stop it (Ctrl+C) once confirmed.

- [ ] **Step 4: Add backend dependencies**

Edit `src-tauri/Cargo.toml` `[dependencies]` to include:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
chrono = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```
`tauri`, `tauri-build`, `tokio`, `serde` are already present from the scaffolder; merge by hand (don't duplicate `serde`). Run:
```bash
cd src-tauri && cargo build && cd ..
```
Expected: compiles, fetching rusqlite/chrono.

- [ ] **Step 5: Add frontend dependencies**

Run:
```bash
npm install uplot
npm install -D vitest @types/uplot
```
Expected: packages added to `package.json`.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: scaffold tauri+svelte+ts app, add deps

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Rust `models` + `config` module (TDD)

**Files:**
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/main.rs` (add `mod config; mod models;`)

**Interfaces:**
- Produces:
  - `models::UsageSummary` — struct with `i64` fields `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`, `request_count`, `unpriced_rows`, and `f64 total_cost_usd`; derives `Serialize, Deserialize, Default, Clone`.
  - `config::Config` — struct with `db_path: Option<String>`, `poll_interval_sec: u64`, `usd_to_cny: f64`, `overlay: WindowState`, `detail: WindowState`.
  - `config::WindowState` — `{ x: i32, y: i32, visible: bool }`.
  - `config::load(app: &AppHandle) -> Config`
  - `config::save(app: &AppHandle, &Config)`
  - `config::resolve_db_path(app: &AppHandle) -> Option<PathBuf>` — returns configured path or `%USERPROFILE%\.cc-switch\cc-switch.db` if it exists, else None.

- [ ] **Step 1: Write `models.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageSummary {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub total_cost_usd: f64,
    pub request_count: i64,
    pub unpriced_rows: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourBucket {
    pub hour: String,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRow {
    pub model: String,
    pub requests: i64,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRow {
    pub provider_id: String,
    pub name: String,
    pub requests: i64,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodayDetail {
    pub hours: Vec<HourBucket>,
    pub by_model: Vec<ModelRow>,
    pub by_provider: Vec<ProviderRow>,
}
```

- [ ] **Step 2: Write the failing `config` tests**

Create `src-tauri/src/config.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self { x: 100, y: 100, visible: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub db_path: Option<String>,
    #[serde(default = "default_poll")]
    pub poll_interval_sec: u64,
    #[serde(default = "default_rate")]
    pub usd_to_cny: f64,
    #[serde(default)]
    pub overlay: WindowState,
    #[serde(default = "default_detail")]
    pub detail: WindowState,
}

fn default_poll() -> u64 { 3 }
fn default_rate() -> f64 { 7.2 }
fn default_detail() -> WindowState { WindowState { x: 800, y: 400, visible: false } }

impl Default for Config {
    fn default() -> Self {
        Config {
            db_path: None,
            poll_interval_sec: 3,
            usd_to_cny: 7.2,
            overlay: WindowState { x: 1600, y: 40, visible: true },
            detail: WindowState { x: 800, y: 400, visible: false },
        }
    }
}

impl Config {
    pub fn from_json(text: &str) -> Config {
        serde_json::from_str(text).unwrap_or_else(|_| Config::default())
    }
}

pub fn load(app: &AppHandle) -> Config {
    match app.path().app_config_dir() {
        Ok(dir) => {
            let p = dir.join("config.json");
            std::fs::read_to_string(&p).map(|t| Config::from_json(&t)).unwrap_or_default()
        }
        Err(_) => Config::default(),
    }
}

pub fn save(app: &AppHandle, cfg: &Config) {
    if let Ok(dir) = app.path().app_config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(text) = serde_json::to_string_pretty(cfg) {
            let _ = std::fs::write(dir.join("config.json"), text);
        }
    }
}

pub fn resolve_db_path(app: &AppHandle) -> Option<PathBuf> {
    let cfg = load(app);
    if let Some(p) = cfg.db_path {
        let pb = PathBuf::from(p);
        if pb.exists() { return Some(pb); }
    }
    let default = home_cc_switch_db()?;
    if default.exists() { Some(default) } else { None }
}

fn home_cc_switch_db() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")?;
    Some(PathBuf::from(home).join(".cc-switch").join("cc-switch.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_json_missing_file_returns_default() {
        let cfg = Config::from_json("");
        assert_eq!(cfg.poll_interval_sec, 3);
        assert!((cfg.usd_to_cny - 7.2).abs() < 1e-9);
        assert!(cfg.db_path.is_none());
    }

    #[test]
    fn from_json_roundtrip() {
        let cfg = Config { db_path: Some("C:/x.db".into()), poll_interval_sec: 5, usd_to_cny: 7.0,
            overlay: WindowState { x: 10, y: 20, visible: true },
            detail: WindowState { x: 30, y: 40, visible: false } };
        let text = serde_json::to_string(&cfg).unwrap();
        let back = Config::from_json(&text);
        assert_eq!(back.db_path.as_deref(), Some("C:/x.db"));
        assert_eq!(back.poll_interval_sec, 5);
        assert!((back.usd_to_cny - 7.0).abs() < 1e-9);
        assert_eq!(back.overlay.x, 10);
        assert_eq!(back.detail.visible, false);
    }

    #[test]
    fn from_json_partial_uses_defaults() {
        let text = r#"{"usd_to_cny":6.5}"#;
        let cfg = Config::from_json(text);
        assert!((cfg.usd_to_cny - 6.5).abs() < 1e-9);
        assert_eq!(cfg.poll_interval_sec, 3); // defaulted
    }

    #[test]
    fn from_json_corrupt_returns_default() {
        let cfg = Config::from_json("{not valid json");
        assert_eq!(cfg.poll_interval_sec, 3);
    }
}
```

- [ ] **Step 3: Wire modules into `main.rs`**

At the top of `src-tauri/src/main.rs` add:
```rust
mod config;
mod models;
```
(Leave the rest of the scaffolded `main` as-is for now.)

- [ ] **Step 4: Run tests to verify they pass**

Run:
```bash
cd src-tauri && cargo test --lib && cd ..
```
Expected: `config::tests::*` 4 tests pass; 0 failures.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/config.rs src-tauri/src/main.rs
git commit -m "feat(config): config load/save with defaults, usage models

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Rust `db` module — read-only connection, midnight, summary (TDD)

**Files:**
- Create: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/main.rs` (add `mod db;`)

**Interfaces:**
- Consumes: `models::UsageSummary`.
- Produces:
  - `db::open_readonly(path: &Path) -> rusqlite::Result<Connection>`
  - `db::midnight_unix(now_unix: i64, offset_seconds: i32) -> i64` (pure, tested)
  - `db::midnight_unix_live() -> i64`
  - `db::fetch_summary(conn: &Connection, since: i64) -> rusqlite::Result<UsageSummary>`

- [ ] **Step 1: Write the failing tests + module**

Create `src-tauri/src/db.rs`:
```rust
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

use crate::models::UsageSummary;

/// Open cc-switch's DB strictly read-only.
pub fn open_readonly(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "query_only", true)?;
    Ok(conn)
}

/// Given current unix seconds and local UTC offset (east positive, seconds),
/// return the unix timestamp of the most recent local midnight.
pub fn midnight_unix(now_unix: i64, offset_seconds: i32) -> i64 {
    let offset = offset_seconds as i64;
    let local = now_unix + offset;
    let secs_into_day = local.rem_euclid(86_400);
    local - secs_into_day - offset
}

pub fn midnight_unix_live() -> i64 {
    use chrono::Local;
    let now = Local::now();
    let ts = now.timestamp();
    let off = now.offset().local_minus_utc();
    midnight_unix(ts, off)
}

pub fn fetch_summary(conn: &Connection, since: i64) -> rusqlite::Result<UsageSummary> {
    let mut stmt = conn.prepare(
        "SELECT \
            COALESCE(SUM(input_tokens),0), \
            COALESCE(SUM(output_tokens),0), \
            COALESCE(SUM(cache_read_tokens),0), \
            COALESCE(SUM(cache_creation_tokens),0), \
            COALESCE(SUM(CAST(total_cost_usd AS REAL)),0), \
            COUNT(*), \
            COALESCE(SUM(CASE WHEN CAST(total_cost_usd AS REAL)=0 THEN 1 ELSE 0 END),0) \
         FROM proxy_request_logs WHERE created_at >= ?1",
    )?;
    let s = stmt.query_row(rusqlite::params![since], |r| {
        Ok(UsageSummary {
            input_tokens: r.get(0)?,
            output_tokens: r.get(1)?,
            cache_read_tokens: r.get(2)?,
            cache_creation_tokens: r.get(3)?,
            total_cost_usd: r.get(4)?,
            request_count: r.get(5)?,
            unpriced_rows: r.get(6)?,
        })
    })?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db_with_rows() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE proxy_request_logs (
                input_tokens INTEGER, output_tokens INTEGER,
                cache_read_tokens INTEGER, cache_creation_tokens INTEGER,
                total_cost_usd TEXT, created_at INTEGER,
                model TEXT, provider_id TEXT, app_type TEXT)",
            [],
        ).unwrap();
        // since=1000; two rows after, one before, one unpriced (cost 0)
        conn.execute("INSERT INTO proxy_request_logs VALUES (100,10,0,0,'0.05',1200,'a','p1','claude')", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (200,20,5,0,'0.10',1300,'b','p1','codex')", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (50,5,0,0,'0',900,'c','p2','claude')", []).unwrap(); // before since, unpriced
        conn.execute("INSERT INTO proxy_request_logs VALUES (300,0,0,0,'0',1400,'glm-5.2','p1','claude')", []).unwrap(); // after, unpriced
        conn
    }

    #[test]
    fn midnight_unix_cst_midday() {
        assert_eq!(midnight_unix(1767240000, 28800), 1767196800);
    }
    #[test]
    fn midnight_unix_cst_just_after_midnight() {
        assert_eq!(midnight_unix(1767198600, 28800), 1767196800);
    }
    #[test]
    fn midnight_unix_negative_offset() {
        assert_eq!(midnight_unix(1767304800, -36000), 1767261600);
    }

    #[test]
    fn fetch_summary_sums_only_rows_since() {
        let conn = mem_db_with_rows();
        let s = fetch_summary(&conn, 1000).unwrap();
        assert_eq!(s.input_tokens, 100 + 200 + 300); // excludes the 50-row
        assert_eq!(s.output_tokens, 10 + 20 + 0);
        assert_eq!(s.cache_read_tokens, 0 + 5 + 0);
        assert_eq!(s.request_count, 3);
        assert!((s.total_cost_usd - 0.15).abs() < 1e-9); // 0.05+0.10+0
        assert_eq!(s.unpriced_rows, 1); // only the glm-5.2 row after since
    }

    #[test]
    fn fetch_summary_empty_returns_zeros() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE proxy_request_logs (input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_creation_tokens INTEGER, total_cost_usd TEXT, created_at INTEGER, model TEXT, provider_id TEXT, app_type TEXT)", []).unwrap();
        let s = fetch_summary(&conn, 1000).unwrap();
        assert_eq!(s.request_count, 0);
        assert_eq!(s.total_cost_usd, 0.0);
        assert_eq!(s.unpriced_rows, 0);
    }
}
```

- [ ] **Step 2: Wire into `main.rs`**

Add `mod db;` to the top of `src-tauri/src/main.rs`.

- [ ] **Step 3: Run tests to verify they pass**

Run:
```bash
cd src-tauri && cargo test --lib && cd ..
```
Expected: all `db::tests::*` and `config::tests::*` pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db.rs src-tauri/src/main.rs
git commit -m "feat(db): read-only sqlite open, local-midnight, today summary

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Rust `db` module — detail query (TDD)

**Files:**
- Modify: `src-tauri/src/db.rs` (add `fetch_detail` + tests)

**Interfaces:**
- Consumes: `models::{TodayDetail, HourBucket, ModelRow, ProviderRow}`, real `providers` table shape `(id, app_type, name, ...)`.
- Produces: `db::fetch_detail(conn: &Connection, since: i64) -> rusqlite::Result<TodayDetail>`

- [ ] **Step 1: Add `fetch_detail` and tests to `db.rs`**

Append inside `db.rs` (above the `#[cfg(test)]` block, or restructure — keep tests together):
```rust
use crate::models::{TodayDetail, HourBucket, ModelRow, ProviderRow};

pub fn fetch_detail(conn: &Connection, since: i64) -> rusqlite::Result<TodayDetail> {
    let mut hours_stmt = conn.prepare(
        "SELECT strftime('%Y-%m-%dT%H', datetime(created_at,'unixepoch','localtime')) AS hour, \
                COALESCE(SUM(input_tokens+output_tokens+cache_read_tokens+cache_creation_tokens),0), \
                COALESCE(SUM(CAST(total_cost_usd AS REAL)),0) \
         FROM proxy_request_logs WHERE created_at >= ?1 GROUP BY hour ORDER BY hour",
    )?;
    let hours: Vec<HourBucket> = hours_stmt.query_map(rusqlite::params![since], |r| {
        Ok(HourBucket { hour: r.get(0)?, tokens: r.get(1)?, cost: r.get(2)? })
    })?.filter_map(Result::ok).collect();

    let mut model_stmt = conn.prepare(
        "SELECT model, COUNT(*), \
                COALESCE(SUM(input_tokens+output_tokens+cache_read_tokens+cache_creation_tokens),0), \
                COALESCE(SUM(CAST(total_cost_usd AS REAL)),0) \
         FROM proxy_request_logs WHERE created_at >= ?1 GROUP BY model ORDER BY 4 DESC",
    )?;
    let by_model: Vec<ModelRow> = model_stmt.query_map(rusqlite::params![since], |r| {
        Ok(ModelRow { model: r.get::<_, Option<String>>(0)?.unwrap_or_default(), requests: r.get(1)?, tokens: r.get(2)?, cost: r.get(3)? })
    })?.filter_map(Result::ok).collect();

    let mut prov_stmt = conn.prepare(
        "SELECT r.provider_id, COALESCE(p.name, r.provider_id), COUNT(*), \
                COALESCE(SUM(r.input_tokens+r.output_tokens+r.cache_read_tokens+r.cache_creation_tokens),0), \
                COALESCE(SUM(CAST(r.total_cost_usd AS REAL)),0) \
         FROM proxy_request_logs r \
         LEFT JOIN providers p ON p.id = r.provider_id AND p.app_type = r.app_type \
         WHERE r.created_at >= ?1 GROUP BY r.provider_id ORDER BY 5 DESC",
    )?;
    let by_provider: Vec<ProviderRow> = prov_stmt.query_map(rusqlite::params![since], |r| {
        Ok(ProviderRow {
            provider_id: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            requests: r.get(2)?,
            tokens: r.get(3)?,
            cost: r.get(4)?,
        })
    })?.filter_map(Result::ok).collect();

    Ok(TodayDetail { hours, by_model, by_provider })
}
```

Add tests inside the `#[cfg(test)]` block:
```rust
    fn mem_db_full() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE proxy_request_logs (input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_creation_tokens INTEGER, total_cost_usd TEXT, created_at INTEGER, model TEXT, provider_id TEXT, app_type TEXT);
             CREATE TABLE providers (id TEXT, app_type TEXT, name TEXT, is_current INTEGER);"
        ).unwrap();
        conn.execute("INSERT INTO providers VALUES ('p1','claude','Bailian',1)", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (100,10,0,0,'0.05',1200,'glm-5.2','p1','claude')", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (200,20,5,0,'0.10',1300,'qwen','p1','claude')", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (50,5,0,0,'0',900,'old','p2','codex')", []).unwrap();
        conn
    }

    #[test]
    fn fetch_detail_groups_by_model() {
        let conn = mem_db_full();
        let d = fetch_detail(&conn, 1000).unwrap();
        assert_eq!(d.by_model.len(), 2); // glm-5.2 + qwen (old excluded)
        let total_tokens: i64 = d.by_model.iter().map(|m| m.tokens).sum();
        assert_eq!(total_tokens, (100+10) + (200+20+5));
        let total_cost: f64 = d.by_model.iter().map(|m| m.cost).sum();
        assert!((total_cost - 0.15).abs() < 1e-9);
    }

    #[test]
    fn fetch_detail_joins_provider_name() {
        let conn = mem_db_full();
        let d = fetch_detail(&conn, 1000).unwrap();
        // p1 has a name (Bailian); the row with provider_id from logs joins on (id, app_type)
        let p1 = d.by_provider.iter().find(|r| r.provider_id == "p1").unwrap();
        assert_eq!(p1.name, "Bailian");
        assert_eq!(p1.requests, 2);
    }

    #[test]
    fn fetch_detail_hours_nonempty() {
        let conn = mem_db_full();
        let d = fetch_detail(&conn, 1000).unwrap();
        assert!(!d.hours.is_empty());
        // hour bucket string shape YYYY-MM-DDTHH
        assert!(d.hours[0].hour.len() == 13);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run:
```bash
cd src-tauri && cargo test --lib && cd ..
```
Expected: all tests pass including the three new `fetch_detail_*`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat(db): today detail (hourly, by-model, by-provider)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Rust poller, commands, and main wiring (windows + tray)

**Files:**
- Create: `src-tauri/src/poller.rs`
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs` (build windows, tray, register commands, spawn poller)
- Modify: `src-tauri/Cargo.toml` (add `tauri-plugin-dialog` if not present)
- Modify: `src-tauri/tauri.conf.json` (window config, identifier, icons)

**Interfaces:**
- Consumes: `config`, `db`, `models`.
- Produces:
  - Tauri event `usage-update` payload `UsageSummary` (emitted every poll tick).
  - Commands: `get_today_detail() -> Option<TodayDetail>`, `save_overlay_position(x: i32, y: i32)`, `save_detail_position(x: i32, y: i32)`, `relocate_db(path: String)`, `show_detail_window()`.

- [ ] **Step 1: Write `poller.rs`**

Emits a `db-status` event every tick (`{ ok: bool, message: String }`) and `usage-update` only on success. This lets the overlay show an explicit "DB 未找到 / 重新定位" state instead of silently staying at `—`.

```rust
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::{config, db};

pub fn spawn(app: AppHandle) {
    tokio::spawn(async move {
        let cfg = config::load(&app);
        let interval = Duration::from_secs(cfg.poll_interval_sec.max(1));
        loop {
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
```

- [ ] **Step 2: Write `commands.rs`**

```rust
use tauri::{AppHandle, Manager, State};
use std::sync::Mutex;

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

#[tauri::command]
pub fn relocate_db(app: AppHandle, path: String) -> bool {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() { return false; }
    let mut cfg = config::load(&app);
    cfg.db_path = Some(path);
    config::save(&app, &cfg);
    true
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
    let path = app.dialog()
        .file()
        .add_filter("SQLite DB", &["db"])
        .pick_file()
        .await
        .map(|f| f.into_path().ok().map(|p| p.to_string_lossy().to_string()))
        .flatten();
    match path {
        Some(p) => Ok(relocate_db(app, p)),
        None => Ok(false),
    }
}
```

Note: `relocate_db` is called directly (not via invoke) from `pick_db_path`. For `#[tauri::command]` async functions calling a sync command, extract the body into a private helper if the borrow checker complains; the simplest fix is to inline the relocate logic in `pick_db_path`. If the compiler rejects calling a `#[tauri::command]` fn directly, refactor `relocate_db`'s body into `fn relocate_db_inner(app, path) -> bool` and have both the command and `pick_db_path` call the inner helper.

- [ ] **Step 3: Rewrite `main.rs` to build windows, tray, and wire everything**

Replace `src-tauri/src/main.rs` contents:
```rust
mod commands;
mod config;
mod db;
mod models;
mod poller;

use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示悬浮窗", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏悬浮窗", true, None::<&str>)?;
    let relocate = MenuItem::with_id(app, "relocate", "重新定位 DB…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &relocate, &quit])?;
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("cc-monitor")
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => { if let Some(w) = app.get_webview_window("overlay") { let _ = w.show(); } }
            "hide" => { if let Some(w) = app.get_webview_window("overlay") { let _ = w.hide(); } }
            "relocate" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = commands::pick_db_path(app).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, _e| {
            let app = tray.app_handle();
            if let Some(w) = app.get_webview_window("overlay") {
                if w.is_visible().unwrap_or(false) { let _ = w.hide(); }
                else { let _ = w.show(); }
            }
        })
        .build(app)?;
    Ok(())
}

fn debounce_save_position(app: tauri::AppHandle, label: &str, x: i32, y: i32) {
    let app = app.clone();
    let label = label.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if label == "overlay" {
            commands::save_overlay_position(app, x, y);
        } else if label == "detail" {
            commands::save_detail_position(app, x, y);
        }
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let cfg = config::load(app.handle());

            let overlay = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::app_path("overlay.html"))
                .title("cc-monitor")
                .inner_size(220.0, 70.0)
                .position(cfg.overlay.x as f64, cfg.overlay.y as f64)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(cfg.overlay.visible)
                .build()?;

            let _detail = WebviewWindowBuilder::new(app, "detail", WebviewUrl::app_path("detail.html"))
                .title("cc-monitor 详情")
                .inner_size(560.0, 440.0)
                .position(cfg.detail.x as f64, cfg.detail.y as f64)
                .visible(false)
                .build()?;

            // Persist window position on move (debounced).
            let app_handle = app.handle().clone();
            overlay.on_window_event(move |e| {
                if let WindowEvent::Moved(pos) = e {
                    debounce_save_position(app_handle.clone(), "overlay", pos.x, pos.y);
                }
            });
            let app_handle2 = app.handle().clone();
            if let Some(detail_win) = app.get_webview_window("detail") {
                detail_win.on_window_event(move |e| {
                    if let WindowEvent::Moved(pos) = e {
                        debounce_save_position(app_handle2.clone(), "detail", pos.x, pos.y);
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
```

- [ ] **Step 4: Add `tauri-plugin-dialog` dependency**

In `src-tauri/Cargo.toml` `[dependencies]` add:
```toml
tauri-plugin-dialog = "2"
```
In `src-tauri/tauri.conf.json`, ensure `"app" -> "windows"` is an empty or removed array (we build windows in code). Keep `"identifier": "com.ccmonitor.desktop"`. Add to `"app"` if a `withGlobalTauri` is desired — leave defaults.

- [ ] **Step 5: Verify it compiles**

Run:
```bash
cd src-tauri && cargo build && cd ..
```
Expected: compiles. If `on_window_event` closure borrow errors arise, adjust to clone `app_handle` per the pattern shown.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/poller.rs src-tauri/src/commands.rs src-tauri/src/main.rs src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "feat(backend): poller, commands, overlay+detail windows, tray

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Frontend — Vite multi-page, format helpers (TDD), overlay window

**Files:**
- Modify: `vite.config.ts` (multi-page input)
- Create: `src/overlay.html`, `src/overlay/Overlay.svelte`
- Create: `src/lib/format.ts`, `src/lib/format.test.ts`
- Modify: `package.json` (add `test` script)
- Modify: `src-tauri/tauri.conf.json` `frontendDist` (already `../dist` from scaffolder; verify)

**Interfaces:**
- Consumes: Tauri event `usage-update` (payload `UsageSummary`), command `show_detail_window`, window JS API `getCurrentWindow().hide()`.
- Produces: a working overlay window showing today's tokens / $ / ¥, draggable, with expand + minimize buttons.

- [ ] **Step 1: Configure Vite multi-page build**

Replace `vite.config.ts`:
```ts
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    rollupOptions: {
      input: {
        index: resolve(__dirname, 'index.html'),
        overlay: resolve(__dirname, 'overlay.html'),
        detail: resolve(__dirname, 'detail.html'),
      },
    },
  },
});
```

- [ ] **Step 2: Write the failing formatter tests**

Create `src/lib/format.test.ts`:
```ts
import { describe, it, expect } from 'vitest';
import { formatTokens, formatUsd, formatCny } from './format';

describe('formatTokens', () => {
  it('shows raw under 1k', () => {
    expect(formatTokens(0)).toBe('0');
    expect(formatTokens(999)).toBe('999');
  });
  it('uses K for thousands', () => {
    expect(formatTokens(1000)).toBe('1K');
    expect(formatTokens(1500)).toBe('1.5K');
    expect(formatTokens(999999)).toBe('1000.0K');
  });
  it('uses M for millions', () => {
    expect(formatTokens(1_000_000)).toBe('1M');
    expect(formatTokens(1_234_567)).toBe('1.2M');
    expect(formatTokens(2_000_000)).toBe('2M');
  });
});

describe('formatUsd', () => {
  it('formats with 2 decimals', () => {
    expect(formatUsd(0)).toBe('$0.00');
    expect(formatUsd(0.4)).toBe('$0.40');
    expect(formatUsd(12.345)).toBe('$12.35');
  });
});

describe('formatCny', () => {
  it('multiplies by rate and formats', () => {
    expect(formatCny(0, 7.2)).toBe('¥0.00');
    expect(formatCny(0.5, 7.2)).toBe('¥3.60');
    expect(formatCny(1, 7)).toBe('¥7.00');
  });
});
```

- [ ] **Step 3: Run the tests to verify they fail**

Add to `package.json` `scripts`:
```json
"test": "vitest run"
```
Run:
```bash
npm test -- src/lib/format.test.ts
```
Expected: FAIL — `Cannot find module './format'`.

- [ ] **Step 4: Write `format.ts`**

```ts
export function formatTokens(n: number): string {
  if (n >= 1_000_000) {
    const v = n / 1_000_000;
    return (Number.isInteger(v) ? v.toString() : v.toFixed(1)) + 'M';
  }
  if (n >= 1_000) {
    const v = n / 1_000;
    return (Number.isInteger(v) ? v.toString() : v.toFixed(1)) + 'K';
  }
  return String(n);
}

export function formatUsd(n: number): string {
  return '$' + n.toFixed(2);
}

export function formatCny(usd: number, rate: number): string {
  return '¥' + (usd * rate).toFixed(2);
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run:
```bash
npm test -- src/lib/format.test.ts
```
Expected: all pass. (`999999/1000 = 999.999` → `toFixed(1)` yields `"1000.0"` → `"1000.0K"`; correct, if slightly ugly — acceptable since real usage rarely sits just under 1M.)

- [ ] **Step 6: Create `overlay.html` and `Overlay.svelte`**

`src/overlay.html`:
```html
<!doctype html>
<html lang="zh">
  <head>
    <meta charset="utf-8" />
    <style> html,body { margin:0; padding:0; background:transparent; font-family: system-ui, sans-serif; } </style>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="./overlay/main.ts"></script>
  </body>
</html>
```

`src/overlay/main.ts`:
```ts
import './app.css';
import Overlay from './Overlay.svelte';
const app = new Overlay({ target: document.getElementById('app')! });
export default app;
```

`src/overlay/Overlay.svelte`:
```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { formatTokens, formatUsd, formatCny } from '../lib/format';

  type UsageSummary = {
    input_tokens: number; output_tokens: number;
    cache_read_tokens: number; cache_creation_tokens: number;
    total_cost_usd: number; request_count: number; unpriced_rows: number;
  };

  let summary: UsageSummary | null = null;
  let usdToCny = 7.2;
  let dbOk = true;
  let dbMessage = '';

  onMount(async () => {
    const s = await invoke<{usd_to_cny:number}>('get_settings');
    usdToCny = s.usd_to_cny;
    const unlistenStatus = await listen<{ok:boolean; message:string}>('db-status', e => {
      dbOk = e.payload.ok; dbMessage = e.payload.message;
    });
    const unlistenUsage = await listen<UsageSummary>('usage-update', e => { summary = e.payload; });
    return () => { unlistenStatus(); unlistenUsage(); };
  });

  const totalTokens = () => summary ? summary.input_tokens + summary.output_tokens + summary.cache_read_tokens + summary.cache_creation_tokens : 0;
  const expand = () => invoke('show_detail_window');
  const hide = () => getCurrentWindow().hide();
  const relocate = () => invoke('pick_db_path');
</script>

<div class="card" data-tauri-drag-region>
  {#if !dbOk}
    <div class="error">
      <span>{dbMessage || 'cc-switch.db 未找到'}</span>
      <button on:click={relocate} data-tauri-drag-region="false">重新定位</button>
    </div>
  {:else}
    <div class="row">
      <span class="label">今日</span>
      <span class="tokens">{summary ? formatTokens(totalTokens()) : '—'}</span>
      <span class="cost">{summary ? formatUsd(summary.total_cost_usd) : '—'}</span>
      <span class="cny">{summary ? formatCny(summary.total_cost_usd, usdToCny) : '—'}</span>
    </div>
    {#if summary && summary.unpriced_rows > 0}
      <div class="warn">⚠ {summary.unpriced_rows} 条未定价</div>
    {/if}
  {/if}
  <div class="btns">
    <button on:click={expand} title="详情">⤢</button>
    <button on:click={hide} title="最小化">—</button>
  </div>
</div>

<style>
  .card { width: 220px; height: 70px; background: rgba(20,20,28,0.85); color: #eee; border-radius: 12px; display:flex; flex-direction:column; justify-content:center; padding: 0 10px; position: relative; -webkit-backdrop-filter: blur(6px); }
  .row { display:flex; align-items:baseline; gap:8px; font-size:13px; }
  .label { color:#9aa; font-size:11px; }
  .tokens { font-weight:600; }
  .cost { margin-left:auto; color:#7cf; }
  .cny { color:#fc9; }
  .warn { font-size:10px; color:#f96; margin-top:2px; }
  .error { display:flex; align-items:center; gap:6px; font-size:11px; color:#f96; }
  .error button { background:#333; border:1px solid #555; color:#eee; border-radius:4px; font-size:10px; padding:2px 6px; cursor:pointer; }
  .btns { position:absolute; top:4px; right:6px; display:flex; gap:2px; }
  .btns button { background:transparent; border:0; color:#bbb; cursor:pointer; font-size:12px; padding:0 4px; }
</style>
```

Add a minimal `src/overlay/app.css`:
```css
* { box-sizing: border-box; }
```

- [ ] **Step 7: Expose `usd_to_cny` to the overlay (optional but clean)**

Add a command in `src-tauri/src/commands.rs`:
```rust
#[tauri::command]
pub fn get_settings(app: AppHandle) -> serde_json::Value {
    let cfg = config::load(&app);
    serde_json::json!({ "usd_to_cny": cfg.usd_to_cny, "poll_interval_sec": cfg.poll_interval_sec })
}
```
Register it in `main.rs` `invoke_handler` (add `commands::get_settings`). In `Overlay.svelte` `onMount`:
```ts
const s = await invoke<{usd_to_cny:number}>('get_settings');
usdToCny = s.usd_to_cny;
```

- [ ] **Step 8: Verify the overlay runs**

Run:
```bash
npm run tauri dev
```
Expected: a small always-on-top dark card appears, showing `—` placeholders initially, then real numbers within ~3s (assuming cc-switch DB has today's rows). Drag works; `⤢` opens (empty) detail window; `—` hides to tray.

- [ ] **Step 9: Run all frontend tests**

Run:
```bash
npm test
```
Expected: format tests pass.

- [ ] **Step 10: Commit**

```bash
git add vite.config.ts src/overlay.html src/overlay src/lib/package.json src-tauri/src/commands.rs src-tauri/src/main.rs
git commit -m "feat(overlay): multi-page vite, format helpers, overlay window

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Frontend — detail window (uPlot chart + tables)

**Files:**
- Create: `src/detail.html`, `src/detail/main.ts`, `src/detail/Detail.svelte`
- Create: `src/lib/events.ts` (shared subscribe helper)

**Interfaces:**
- Consumes: command `get_today_detail` → `TodayDetail`; event `usage-update` (to trigger refresh).
- Produces: detail window with hourly line chart and per-model / per-provider tables.

- [ ] **Step 1: Create `detail.html` and `detail/main.ts`**

`src/detail.html`:
```html
<!doctype html>
<html lang="zh">
  <head><meta charset="utf-8" /><title>cc-monitor 详情</title></head>
  <body>
    <div id="app"></div>
    <script type="module" src="./detail/main.ts"></script>
  </body>
</html>
```

`src/detail/main.ts`:
```ts
import Detail from './Detail.svelte';
const app = new Detail({ target: document.getElementById('app')! });
export default app;
```

- [ ] **Step 2: Write `Detail.svelte`**

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import uPlot from 'uplot';
  import 'uplot/dist/uPlot.min.css';

  type HourBucket = { hour: string; tokens: number; cost: number };
  type ModelRow = { model: string; requests: number; tokens: number; cost: number };
  type ProviderRow = { provider_id: string; name: string; requests: number; tokens: number; cost: number };
  type TodayDetail = { hours: HourBucket[]; by_model: ModelRow[]; by_provider: ProviderRow[] };

  let detail: TodayDetail | null = null;
  let chartEl: HTMLDivElement;
  let plot: uPlot | null = null;
  let unlisten: (() => void) | null = null;
  let refreshTimer: number | null = null;

  async function refresh() {
    detail = await invoke<TodayDetail | null>('get_today_detail');
    renderChart();
  }

  function renderChart() {
    if (!detail || !chartEl) return;
    const labels = detail.hours.map(h => h.hour);
    const x = labels.map((_, i) => i);
    const tokens = detail.hours.map(h => h.tokens);
    const cost = detail.hours.map(h => h.cost);
    const data = [x, tokens, cost];
    if (plot) { plot.setData(data); return; }
    plot = new uPlot({
      width: 520, height: 200,
      series: [
        {},
        { label: 'tokens', stroke: '#7cf', scale: 'tokens' },
        { label: 'cost $', stroke: '#fc9', scale: 'cost', side: 1 },
      ],
      scales: { tokens: { side: 0 }, cost: { side: 1 } },
    }, data, chartEl);
  }

  onMount(async () => {
    await refresh();
    unlisten = await listen('usage-update', () => {
      if (refreshTimer) window.clearTimeout(refreshTimer);
      refreshTimer = window.setTimeout(refresh, 500);
    });
  });
  onDestroy(() => { unlisten?.(); plot?.destroy(); });
</script>

<h2>今日明细</h2>
<div class="chart" bind:this={chartEl}></div>

<h3>按模型</h3>
<table>
  <thead><tr><th>模型</th><th>请求数</th><th>tokens</th><th>$</th></tr></thead>
  <tbody>
    {#if detail}
      {#each detail.by_model as m}
        <tr><td>{m.model}</td><td>{m.requests}</td><td>{m.tokens}</td><td>{m.cost.toFixed(4)}</td></tr>
      {/each}
    {/if}
  </tbody>
</table>

<h3>按 Provider</h3>
<table>
  <thead><tr><th>Provider</th><th>请求数</th><th>tokens</th><th>$</th></tr></thead>
  <tbody>
    {#if detail}
      {#each detail.by_provider as p}
        <tr><td>{p.name}</td><td>{p.requests}</td><td>{p.tokens}</td><td>{p.cost.toFixed(4)}</td></tr>
      {/each}
    {/if}
  </tbody>
</table>

<style>
  body { font-family: system-ui, sans-serif; padding: 12px; }
  table { border-collapse: collapse; width: 100%; font-size: 13px; }
  th, td { border: 1px solid #ddd; padding: 4px 8px; text-align: left; }
  .chart { margin: 8px 0; }
</style>
```

- [ ] **Step 3: Verify the detail window**

Run:
```bash
npm run tauri dev
```
Open detail via overlay `⤢`. Expected: chart + two tables populated from today's DB rows; numbers refresh every ~3s while visible.

- [ ] **Step 4: Commit**

```bash
git add src/detail.html src/detail src/lib
git commit -m "feat(detail): hourly chart + per-model/per-provider tables

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Integration smoke test + production build

**Files:** none (verification + build config).

**Interfaces:** final verification against the spec.

- [ ] **Step 1: Run the full test suite**

Run:
```bash
npm test
cd src-tauri && cargo test --lib && cd ..
```
Expected: all Rust + frontend tests pass.

- [ ] **Step 2: Manual smoke test against live cc-switch**

With cc-switch running (proxy on):
1. `npm run tauri dev`.
2. Overlay shows today's token + $ + ¥; numbers tick up within 3s after a Claude request.
3. Drag overlay to a new position; restart the app; position is restored.
4. `⤢` opens detail window with chart + tables.
5. Tray left-click toggles overlay; right-click menu items work; `退出` quits.
6. When today has unpriced rows (e.g. `glm-5.2`), overlay shows the ⚠ hint.
7. Temporarily rename `~/.cc-switch/cc-switch.db` → overlay switches to the "cc-switch.db 未找到 [重新定位]" state within ~3s; click 重新定位 (or tray → 重新定位 DB…), pick the file, and the overlay returns to live numbers.

- [ ] **Step 3: Build the installer**

Run:
```bash
npm run tauri build
```
Expected: produces an MSI/NSIS installer under `src-tauri/target/release/bundle/`.

- [ ] **Step 4: Commit any build-config tweaks**

```bash
git add -A
git commit -m "chore: build config + smoke verification

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Done

The app is complete when: overlay shows live today-totals, detail panel shows hourly chart + breakdowns, position persists, tray works, tests pass, and an installer builds.
