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

  async function loadSettings() {
    try {
      const settings = await invoke<{ usd_to_cny: number }>("get_settings");
      usdToCny = settings.usd_to_cny;
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

    return () => {
      unlistenDbStatus.then((f) => f());
      unlistenUsageUpdate.then((f) => f());
    };
  });
</script>

{#if !dbOk}
  <div class="error-state">
    <span>{dbMessage || "cc-switch.db 未找到"}</span>
    <button on:click={pickDbPath}>重新定位</button>
  </div>
{:else}
  <div class="card" on:mousedown={startDrag}>
    <div class="screen">
      <div class="line tokens">
        {summary ? formatTokens(summary.input_tokens + summary.output_tokens + summary.cache_read_tokens + summary.cache_creation_tokens) : "—"}
      </div>
      <button
        class="line cny clickable"
        on:click={openDetailWindow}
        title="Click to open detail window"
      >
        {summary ? formatCny(summary.total_cost_usd, usdToCny) : "—"}
      </button>
    </div>
  </div>
{/if}

<style src="./app.css"></style>
