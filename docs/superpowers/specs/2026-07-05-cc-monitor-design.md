# cc-monitor — Windows 悬浮窗实时显示 cc-switch token 消耗与成本

- 日期: 2026-07-05
- 状态: 已设计，待评审

## 1. 目标与范围

构建一个 Windows 桌面悬浮窗应用，从本地 cc-switch 的 SQLite 数据库读取代理请求日志，实时显示今日累计的 token 消耗与成本。

### 范围内
- 单个 Tauri 2.x 桌面应用，面向 Windows。
- 始终置顶的紧凑悬浮窗（overlay），显示今日总 token、今日总成本（$ 与 ¥）。
- 可展开的详细面板窗口：今日按小时折线图、按模型/按 provider 明细表。
- 定时轮询 SQLite DB（默认 3 秒）刷新。
- 可拖动悬浮窗，位置跨启动记忆。
- 系统托盘图标，可显示/隐藏悬浮窗、退出。
- 跨所有 app_type（claude / codex / gemini）合并统计今日数据。

### 范围外（YAGNI）
- 不做历史趋势回溯（仅今日）。
- 不做预算告警/限额（cc-switch 自身已有 limit_daily_usd / limit_monthly_usd）。
- 不做实时速率（tokens/s）显示。
- 不做多平台（仅 Windows）。
- 不做自动汇率抓取。

## 2. 数据源

cc-switch 是一个 Tauri 桌面应用，作为 Claude/Codex/Gemini API 的本地代理，将每个请求的 usage 记录到 SQLite DB。

- **DB 路径**: `%USERPROFILE%\.cc-switch\cc-switch.db`（可配置覆盖）。
- **核心表**:
  - `proxy_request_logs` — 每个请求一行，含 token 与成本明细。关键字段：`input_tokens`、`output_tokens`、`cache_read_tokens`、`cache_creation_tokens`、`total_cost_usd`（TEXT，存十进制数）、`model`、`provider_id`、`app_type`、`status_code`、`session_id`、`created_at`（unix 秒）、`data_source`。
  - `providers` — `id`、`name`、`app_type`、`is_current`。
  - `model_pricing` — 定价表（仅用于参考，成本已在日志行预算好）。
  - `usage_daily_rollups` — 预聚合日表，本应用不依赖（直接查原始日志以保证实时性）。

### "今日"定义
本地时区（Windows 本地时间）当天 0 点到现在的 unix 秒区间。`created_at` 为 unix 秒，与本地时区换算后取当天 0 点。

### 成本口径
- 直接累加 `CAST(total_cost_usd AS REAL)`。
- **已知偏差**：cc-switch 日志中 `[USG-002] 模型定价未找到` 的模型（如 `glm-5.2`）其 `total_cost_usd` 记为 `0`，导致显示的总成本**偏低**。应用检测到今日存在此类行时，显示警告提示，但不臆造成本。

## 3. 架构

技术栈锁定为 **Tauri 2.x (Rust + WebView)**。采用 Approach A：Rust 后端轮询并推送事件，前端订阅。

### 3.1 模块划分

```
cc-monitor/
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── main.rs         # Tauri 入口、窗口/托盘装配
│   │   ├── db.rs           # 只读 SQLite 连接 + 聚合查询
│   │   ├── poller.rs       # tokio 定时轮询任务，发射 Tauri 事件
│   │   ├── config.rs       # 配置加载/保存
│   │   ├── commands.rs     # Tauri 命令（get_today_detail 等）
│   │   └── models.rs       # 数据结构（UsageSummary, TodayDetail, ...）
│   └── tauri.conf.json
├── src/                    # 前端 (Svelte 5 + Vite + TS)
│   ├── overlay/            # 悬浮窗页面
│   ├── detail/             # 详细面板页面
│   ├── lib/                # 格式化、事件订阅
│   └── main.ts
└── docs/
```

### 3.2 后端职责

- **db.rs**: 用 `rusqlite`（bundled feature，自带 SQLite 编译，避免依赖系统 sqlite3.dll）。连接字符串使用只读 URI：`file:<path>?mode=ro`，并执行 `PRAGMA query_only=1`。提供两个查询函数：
  - `fetch_today_summary()` — 返回 `UsageSummary`。
  - `fetch_today_detail()` — 返回 `TodayDetail`（小时桶序列 + 按模型/按 provider 明细）。
- **poller.rs**: 一个 `tokio::spawn` 的循环任务，每 `pollIntervalSec` 秒调用 `fetch_today_summary()`，通过 `app.emit("usage-update", summary)` 推送。错误时保留上一次值并在下次重试。
- **commands.rs**: 暴露 `#[tauri::command] get_today_detail() -> TodayDetail`，供详细面板按需调用。
- **config.rs**: 读写 `%APPDATA%\cc-monitor\config.json`。
- **main.rs**: 创建两个窗口（overlay 默认可见，detail 默认隐藏），托盘，启动 poller。

### 3.3 前端职责

- 两个独立 HTML 入口：`overlay` 与 `detail`（Tauri 多窗口）。
- overlay 监听 `usage-update` 事件更新数字；提供拖动、展开、最小化按钮。
- detail 在 `onMount` 调 `get_today_detail`，并订阅 `usage-update` 触发重新拉取（或仅在可见时拉取）。
- 数字格式化：token 用 `1.2M / 3.4K`，成本 `$0.42` / `¥3.02`。

## 4. 数据流

1. 启动 → config.rs 加载配置（含 db 路径、轮询间隔、汇率、窗口位置）。
2. main.rs 校验 db 路径；不可用则 overlay 显示错误态，托盘菜单提供"重新定位 DB"。
3. poller 每 3s 查 `proxy_request_logs` 今日聚合 → emit `usage-update`。
4. overlay 前端订阅事件，更新 token/$/¥ 数值。
5. 用户点"展开"→ 打开 detail 窗口 → 调 `get_today_detail` → 渲染折线图 + 表格。
6. detail 可见期间，复用 `usage-update` 事件触发 `get_today_detail` 刷新（debounce 500ms）。

### 4.1 聚合查询（summary）

```sql
SELECT
  COALESCE(SUM(input_tokens),0)               AS input_tokens,
  COALESCE(SUM(output_tokens),0)              AS output_tokens,
  COALESCE(SUM(cache_read_tokens),0)          AS cache_read_tokens,
  COALESCE(SUM(cache_creation_tokens),0)      AS cache_creation_tokens,
  COALESCE(SUM(CAST(total_cost_usd AS REAL)),0) AS total_cost_usd,
  COUNT(*)                                    AS request_count,
  SUM(CASE WHEN CAST(total_cost_usd AS REAL)=0 THEN 1 ELSE 0 END) AS unpriced_rows
FROM proxy_request_logs
WHERE created_at >= :today_midnight_unix;
```

### 4.2 明细查询（detail）

- 小时桶：`strftime('%Y-%m-%dT%H', datetime(created_at,'unixepoch','localtime'))` 作为桶键，按桶聚合 token 与成本（仅今日本地时区）。
- 按模型：`GROUP BY model`，输出 `model, requests, input+output+cache tokens, cost`。
- 按 provider：`GROUP BY provider_id`，join `providers` 取 `name`。

## 5. 窗口与交互

### 5.1 overlay 窗口
- frameless，`alwaysOnTop`，`decorations: false`，`transparent: true`，`skipTaskbar: true`。
- 默认尺寸 ~200×64px，圆角半透明卡片。
- 内容：`今日` 标签 · 总 token（紧凑） · `$0.42` · `¥3.02`。
- 整个卡片可拖动（自定义 data-tauri-drag-region）。拖动结束 500ms 后保存位置。
- 右上两个小按钮：`⤢`（展开详细面板）、`—`（最小化到托盘）。
- 当 `unpriced_rows > 0`：在卡片底部显示一行小字 `⚠ N 条未定价`。

### 5.2 detail 窗口
- 普通窗口（非置顶），~520×420px，带标题栏。
- 内容：今日按小时折线图（uPlot，双轴：token 与成本）、按模型表、按 provider 表。
- 可见时每 3s 刷新。

### 5.3 托盘
- 图标：固定 logo（或运行时绘制当前今日成本数字，作为后续增强，MVP 用固定图标）。
- 菜单：`显示悬浮窗` / `隐藏悬浮窗` / `退出`。
- 左键单击 = 切换悬浮窗可见性。

## 6. 配置

`%APPDATA%\cc-monitor\config.json`：
```json
{
  "dbPath": null,
  "pollIntervalSec": 3,
  "usdToCny": 7.2,
  "overlay": { "x": 1600, "y": 40, "visible": true },
  "detail":  { "x": 800,  "y": 400 }
}
```
- `dbPath` 为 null 时使用默认 `%USERPROFILE%\.cc-switch\cc-switch.db`。
- 位置在拖动结束后 debounce 500ms 写入。

## 7. 错误处理

- DB 路径不存在 / 非 SQLite → overlay 显示 `cc-switch.db 未找到`；托盘菜单 `重新定位…` 触发文件选择器，结果写入 config。
- 查询失败（如 DB 被独占锁，理论上 WAL 下不会发生）→ 保留上次值，下次重试，不崩溃。
- `unpriced_rows > 0` → 显示 ⚠ 提示，不阻塞功能。
- 配置文件损坏 → 回退默认值，重写文件。

## 8. 测试

- **Rust 单测**:
  - `db.rs` 聚合函数：用内存 SQLite（`:memory:`）插入若干样本行（含 `total_cost_usd=0` 的未定价行、跨时区边界），断言 summary 与 detail 输出。
  - "今日 0 点" unix 计算：固定一个时间戳，断言换算。
- **前端单测 (Vitest)**:
  - 数字格式化（`1234567 → 1.2M`、`0.42 → $0.42`）。
  - config 读写 mock。
- **手动冒烟**：与 cc-switch 同时运行，发起几次 Claude 请求，观察数字在 3s 内上升；展开详细面板查看图表与表格；拖动悬浮窗、重启应用验证位置记忆；托盘菜单各项。

## 9. 构建与分发

- `npm run tauri build` 产出 MSI/NSIS 安装包。
- 开发：`npm run tauri dev`。
- 依赖：Node.js、Rust toolchain（用户机器已具备 Rust toolchain 需求待确认；若无可由 Tauri 引导安装）。

## 10. 待评审的默认决策（请确认）

1. **汇率**：固定 `usdToCny = 7.2`，仅手动配置，不自动抓取。
2. **轮询间隔**：默认 3 秒。
3. **未定价行**：显示警告，不臆造成本。
4. **detail 窗口**：非置顶、带标题栏。
5. **托盘图标**：MVP 用固定 logo，不绘制动态成本数字。
6. **不依赖 `usage_daily_rollups`**：直接查原始日志保证实时。
