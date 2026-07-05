# CLAUDE.md — cc-monitor

A Windows always-on-top Tauri 2.x overlay that reads the local **cc-switch** SQLite DB and shows today's cumulative token usage and cost (tokens / $ / ¥), with an expandable detail panel (hourly chart + per-model/per-provider tables) and a system tray icon.

## Quick start

```bash
# Rust must be on PATH for tauri commands (rustup didn't add a profile script
# that git-bash picks up, so source it explicitly each shell):
export PATH="$HOME/.cargo/bin:$PATH"

npm install                       # first time only
npm run tauri dev                 # dev build + overlay window
npm run tauri build               # release MSI + NSIS installers
```

Installers land in `src-tauri/target/release/bundle/{msi,nsis}/`.

### Test

```bash
npm test                          # 12 vitest format tests
(cd src-tauri && cargo test --lib)# 12 rust tests: config, midnight, summary, detail
```

24/24 expected. A stale `cc-monitor.exe` or vite holding port 1420 will block `tauri dev` — kill with `taskkill //PID <pid> //F` (check `netstat -ano | grep :1420`).

## Data source

cc-switch is a separate Tauri app that proxies Claude/Codex/Gemini API calls and logs each to a SQLite DB at `%USERPROFILE%\.cc-switch\cc-switch.db`. Read-only access only.

Tables used (see `src-tauri/src/db.rs` for the exact queries):
- **`proxy_request_logs`** — one row per request. `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`, `total_cost_usd` (TEXT, cast to REAL), `model`, `provider_id`, `app_type`, `status_code`, `session_id`, `created_at` (unix seconds). ~25k rows.
- **`providers`** — `(id, app_type, name, is_current)`. Composite PK `(id, app_type)` — JOIN on both.
- `model_pricing`, `usage_daily_rollups` exist but are **not** used (we query raw logs for real-time data).

"Today" = local-midnight → now, computed from `created_at` + the local UTC offset (see `db::midnight_unix` — a pure, unit-tested helper; `midnight_unix_live` wraps it with `chrono::Local`).

**Cost honesty:** `total_cost_usd` is summed as-is. Models without a pricing entry (e.g. `glm-5.2`) are recorded as `0` by cc-switch, so the displayed total **under-reports** true cost. The backend counts these as `unpriced_rows`. (A ⚠ hint was removed from the overlay per user preference — `unpriced_rows` is still computed and shipped in the payload if you want to re-surface it.)

## Architecture

Rust backend polls the DB every 3s and pushes Tauri events; the Svelte frontend subscribes. Two windows, both built **in code** (not in `tauri.conf.json`):

- `overlay` — frameless, `always_on_top`, `transparent`, `skip_taskbar`, 220×70. `overlay.html` at project root.
- `detail` — normal window, 560×440, hidden until opened. `detail.html` at root.

Plain **Svelte 5 + Vite multi-page** (NOT SvelteKit — the `create-tauri-app` `svelte-ts` template is SvelteKit by default; this project was converted to plain Svelte because multi-window Tauri + multi-page HTML entries is simpler without SvelteKit's router).

### Backend (`src-tauri/src/`)

| File | Responsibility |
|------|----------------|
| `main.rs` | Thin: `#![windows_subsystem]` + `cc_monitor_lib::run()`. **Do not put modules/logic here.** |
| `lib.rs` | `mod` declarations + `run()` with the `tauri::Builder` (windows, tray, poller, command handler). |
| `models.rs` | Serde structs: `UsageSummary`, `TodayDetail`, `HourBucket`, `ModelRow`, `ProviderRow`. Field names are the wire contract the frontend reads — keep them snake_case. |
| `config.rs` | Load/save `%APPDATA%\cc-monitor\config.json`; `resolve_db_path` (configured path → fallback `%USERPROFILE%\.cc-switch\cc-switch.db`). |
| `db.rs` | `open_readonly`, `midnight_unix(_live)`, `fetch_summary`, `fetch_detail`. All SQL parameterized (`?1`); uses in-memory SQLite for tests. |
| `poller.rs` | `tauri::async_runtime::spawn` loop; emits `db-status` (`{ok,message}`) every tick and `usage-update` (`UsageSummary`) on success. |
| `commands.rs` | `#[tauri::command]`s: `get_today_detail`, `get_settings`, `show_detail_window`, `pick_db_path`, `relocate_db` (+ `relocate_db_inner` helper), `save_overlay_position`, `save_detail_position`. |

### Frontend (`src/`)

| File | Responsibility |
|------|----------------|
| `overlay/{Overlay.svelte,main.ts,app.css}` | Overlay window. Subscribes to `db-status` + `usage-update`; calls `get_settings` on mount. `main.ts` uses Svelte 5 `mount()`. |
| `detail/{Detail.svelte,main.ts}` | Detail window. `onMount` → `invoke('get_today_detail')`; uPlot dual-axis chart + 2 tables; debounced refresh on `usage-update`; `onDestroy` cleans up listener + plot + timer. |
| `lib/format.ts` | `formatTokens` (K/M), `formatUsd` ($x.xx), `formatCny` (¥x.xx). Vitest-tested. |
| `*.html` at project root | Vite multi-page entries (`overlay.html`, `detail.html`, `index.html`). **Must stay at root** — `WebviewUrl::App("overlay.html")` resolves to `dist/overlay.html`; if these move under `src/`, the build nests at `dist/src/overlay.html` and the windows load blank. |

### Event/command contract (backend ↔ frontend)

- Event `usage-update` → `UsageSummary` (`input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`, `total_cost_usd`, `request_count`, `unpriced_rows`).
- Event `db-status` → `{ ok: boolean, message: string }` (frontend reads `ok`, **not** `found`).
- Commands: `get_today_detail`, `get_settings` (`{usd_to_cny, poll_interval_sec}`), `show_detail_window`, `pick_db_path` (async, returns real bool), `relocate_db`, `save_overlay_position`, `save_detail_position`.

Changing a field name on either side without the other is the #1 cross-cutting bug class — a prior review missed `tokens`/`cost_usd` (frontend) vs `input_tokens`/`total_cost_usd` (backend) and the overlay rendered NaN.

## Config

`%APPDATA%\cc-monitor\config.json`:
```json
{ "dbPath": null, "pollIntervalSec": 3, "usdToCny": 7.2,
  "overlay": { "x": 1600, "y": 40, "visible": true },
  "detail":  { "x": 800,  "y": 400, "visible": false } }
```
`dbPath: null` → use cc-switch default. Window positions auto-persist on drag (debounced, via `WindowEvent::Moved`).

## Capabilities (`src-tauri/capabilities/default.json`)

Scoped to `["overlay", "detail"]`. `core:default` alone is **not enough**:
- `core:window:allow-start-dragging` — required for drag (`core:window:default` does NOT include it).
- `core:window:allow-hide` — for the hide/minimize path.

If a frontend `listen()` or window op silently does nothing, check this file first — capabilities are baked at compile time (`generate_context!`), so a change needs a rebuild.

## Tauri 2 gotchas (hard-won)

1. **`tokio::spawn` panics** in a Tauri context — use `tauri::async_runtime::spawn` for the poller and any async tasks.
2. **`data-tauri-drag-region` is unreliable** on a transparent frameless window on Windows. The overlay uses the explicit `getCurrentWindow().startDragging()` API on `mousedown` instead (skips when the press lands on the cost button). Keep this pattern.
3. **`tauri-plugin-dialog`'s `pick_file()` is callback-only** (no `.await`). `pick_db_path` blocks on an `mpsc::channel` wrapped in `tauri::async_runtime::spawn_blocking` so it returns a real `Result<bool>` without pinning a tokio worker thread.
4. **Svelte 5:** use `mount(Component, { target })`, not `new Component()`. Don't mix `on:event` and `onevent` syntaxes in one component.
5. **Window labels** (`overlay`, `detail`) are created in `lib.rs` — never redeclare them in `tauri.conf.json` (Tauri panics "label already exists").

## Conventions

- **TDD** for pure logic: `format.ts` (vitest), `config.rs`/`db.rs` (cargo test, in-memory SQLite). UI wiring isn't unit-tested.
- **Commit style:** conventional commits (`feat:`, `fix:`, `chore:`, `docs:`). End every commit with:
  ```
  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  ```
- **Read-only DB:** `OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` + `PRAGMA query_only=1`. Never write to cc-switch's DB.

## Design docs & history

- Spec: `docs/superpowers/specs/2026-07-05-cc-monitor-design.md`
- Plan: `docs/superpowers/plans/2026-07-05-cc-monitor.md`
- SDD progress ledger: `.superpowers/sdd/progress.md` (per-task status, commit ranges, gotchas discovered)
- Per-task reports: `.superpowers/sdd/task-*-report.md`

Read the ledger first if resuming work — it records which tasks are done and the non-obvious fixes (tokio panic, field-name mismatches, capability scope, drag API).
