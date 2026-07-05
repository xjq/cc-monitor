<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { formatTokens, formatCny } from "../lib/format";

  interface UsageSummary {
    input_tokens: number;
    output_tokens: number;
    cache_read_tokens: number;
    cache_creation_tokens: number;
    total_cost_usd: number;
    request_count: number;
    unpriced_rows: number;
  }

  let summary: UsageSummary | null = null;
  let dbOk = false;
  let dbMessage = "";
  let usdToCny: number = 7.2;
  let fontScale: string = "medium";

  async function loadSettings() {
    try {
      const settings = await invoke<{ usd_to_cny: number; font_scale: string }>("get_settings");
      usdToCny = settings.usd_to_cny;
      fontScale = settings.font_scale || "medium";
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  function handleDbStatus(event: { payload: { ok: boolean; message: string } }) {
    dbOk = event.payload.ok;
    dbMessage = event.payload.message || "";
  }

  async function handleUsageUpdate(event: any) {
    summary = event.payload;
  }

  function handleFontScale(event: { payload: string }) {
    fontScale = event.payload || "medium";
  }

  async function openDetailWindow() {
    try {
      await invoke("show_detail_window");
    } catch (e) {
      console.error("Failed to open detail window:", e);
    }
  }

  async function pickDbPath() {
    try {
      await invoke("pick_db_path");
    } catch (e) {
      console.error("Failed to pick DB path:", e);
    }
  }

  // Explicit drag: more robust than the data-tauri-drag-region attribute.
  // Skip when the press lands on the interactive cost button (let its click fire).
  async function startDrag(e: MouseEvent) {
    if ((e.target as HTMLElement)?.closest("button")) return;
    try {
      await getCurrentWindow().startDragging();
    } catch (err) {
      console.error("startDragging failed:", err);
    }
  }

  onMount(() => {
    loadSettings();

    const unlistenDbStatus = listen("db-status", handleDbStatus);
    const unlistenUsageUpdate = listen("usage-update", handleUsageUpdate);
    const unlistenFontScale = listen("font-scale-changed", handleFontScale);

    return () => {
      unlistenDbStatus.then((f) => f());
      unlistenUsageUpdate.then((f) => f());
      unlistenFontScale.then((f) => f());
    };
  });
</script>

{#if !dbOk}
  <div class="error-state">
    <span>{dbMessage || "cc-switch.db 未找到"}</span>
    <button on:click={pickDbPath}>重新定位</button>
  </div>
{:else}
  <div class="card size-{fontScale}" on:mousedown={startDrag}>
    <div class="screen">
      <div class="line tokens">
        {summary ? formatTokens(summary.input_tokens + summary.output_tokens + summary.cache_read_tokens + summary.cache_creation_tokens) : "—"}
      </div>
      <div class="line cny">
        {summary ? formatCny(summary.total_cost_usd, usdToCny) : "—"}
      </div>
    </div>
    <button
      class="key"
      on:click={openDetailWindow}
      title="展开详情"
      aria-label="展开详情"
    >≡</button>
  </div>
{/if}

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  :global(body) {
    font-family: "Consolas", "Courier New", "Lucida Console", monospace;
    background: transparent;
    overflow: hidden;
    user-select: none;
    cursor: grab;
  }

  :global(body:active) {
    cursor: grabbing;
  }

  /* Outer shell = calculator body / bezel around the LCD.
     Sizes are driven by per-scale CSS vars (see .size-* below). */
  .card {
    width: var(--w, 320px);
    height: var(--h, 180px);
    padding: 8px;
    border-radius: 10px;
    background: linear-gradient(180deg, #2c2c2c, #161616);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  /* Font-scale presets. Keep --w/--h in sync with overlay_size_for() in lib.rs. */
  .size-small {
    --w: 192px; --h: 128px;
    --tok: 20px; --cny: 36px;
    --key: 22px; --keyh: 32px; --keymw: 80px; --keyp: 14px;
  }
  .size-medium {
    --w: 240px; --h: 160px;
    --tok: 26px; --cny: 46px;
    --key: 28px; --keyh: 40px; --keymw: 96px; --keyp: 18px;
  }
  .size-large {
    --w: 288px; --h: 192px;
    --tok: 32px; --cny: 56px;
    --key: 34px; --keyh: 48px; --keymw: 112px; --keyp: 22px;
  }

  .error-state {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: linear-gradient(180deg, #2c2c2c, #161616);
    color: #ff6b6b;
    border-radius: 8px;
    font-size: 12px;
  }

  .error-state button {
    background: #4a9eff;
    color: white;
    border: none;
    border-radius: 4px;
    padding: 2px 8px;
    cursor: pointer;
    font-size: 11px;
  }

  /* The LCD glass: recessed, olive-gray, with an inner bevel + glare. */
  .screen {
    position: relative;
    flex: 1;
    width: 100%;
    padding: 10px 14px;
    border-radius: 4px;
    background: #a8b59a;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
    gap: 8px;
    box-shadow:
      inset 0 0 0 2px rgba(0, 0, 0, 0.12),
      inset 1px 1px 4px rgba(0, 0, 0, 0.35),
      inset -1px -1px 2px rgba(255, 255, 255, 0.25);
  }

  /* Faint top glare to sell the glass look. */
  .screen::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 3px;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.1), transparent 45%);
    pointer-events: none;
  }

  /* 7-seg-ish digit rows: dark on LCD, monospaced, with a ghost shadow. */
  .line {
    font-size: 32px;
    font-weight: 700;
    color: #0f1a0f;
    letter-spacing: 3px;
    line-height: 1.05;
    font-variant-numeric: tabular-nums;
    text-shadow: 1px 0 0 rgba(15, 26, 15, 0.18);
  }

  .line.tokens {
    font-size: var(--tok, 28px);
    opacity: 0.78;
  }

  .line.cny {
    font-size: var(--cny, 52px);
  }

  /* Calculator key below the LCD — opens the detail window. */
  .key {
    align-self: flex-end;
    height: var(--keyh, 44px);
    min-width: var(--keymw, 104px);
    padding: 0 var(--keyp, 20px);
    border: none;
    border-radius: 6px;
    background: linear-gradient(180deg, #4a4a4a, #2a2a2a);
    color: #c8d8c0;
    font-family: inherit;
    font-size: var(--key, 30px);
    font-weight: 700;
    line-height: 1;
    letter-spacing: 2px;
    cursor: pointer;
    box-shadow:
      0 2px 4px rgba(0, 0, 0, 0.55),
      inset 0 1px 0 rgba(255, 255, 255, 0.18);
  }

  .key:hover {
    color: #e8f5e0;
  }

  .key:active {
    background: linear-gradient(180deg, #232323, #161616);
    box-shadow:
      0 1px 1px rgba(0, 0, 0, 0.55),
      inset 0 1px 2px rgba(0, 0, 0, 0.5);
  }
</style>
