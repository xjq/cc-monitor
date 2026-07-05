<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { formatTokens, formatUsd, formatCny } from "../lib/format";

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
  let showCostCny: boolean = false;

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
  <div class="card" data-tauri-drag-region>
    {#if summary && summary.unpriced_rows > 0}
      <div class="warn">⚠ {summary.unpriced_rows} 条未定价</div>
    {/if}
    <div class="metric">
      <span class="metric-label">Tokens</span>
      <span class="metric-value">{summary ? formatTokens(summary.input_tokens + summary.output_tokens + summary.cache_read_tokens + summary.cache_creation_tokens) : "—"}</span>
    </div>
    <div class="metric">
      <span class="metric-label">Cost</span>
      <button
        class="metric-value clickable"
        on:click={openDetailWindow}
        title="Click to open detail window"
        data-tauri-drag-region="false"
      >
        {#if summary}
          {#if showCostCny}
            {formatCny(summary.total_cost_usd, usdToCny)}
          {:else}
            {formatUsd(summary.total_cost_usd)}
          {/if}
        {:else}
          —
        {/if}
      </button>
    </div>
  </div>
{/if}

<style src="./app.css"></style>
