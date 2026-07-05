<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { formatTokens, formatHours, formatUsd, formatCny } from "../lib/format";

  interface Summary {
    tokens: number;
    cost_usd: number;
    hours: number;
  }

  let summary: Summary | null = null;
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

  async function handleDbStatus(event: any) {
    dbOk = event.payload.found === true;
    dbMessage = event.payload.message || "";
    if (event.payload.found) {
      summary = event.payload.summary;
    }
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
    <div class="metric">
      <span class="metric-label">Tokens</span>
      <span class="metric-value">{summary ? formatTokens(summary.tokens) : "—"}</span>
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
            {formatCny(summary.cost_usd, usdToCny)}
          {:else}
            {formatUsd(summary.cost_usd)}
          {/if}
        {:else}
          —
        {/if}
      </button>
    </div>
    <div class="metric">
      <span class="metric-label">Hours</span>
      <span class="metric-value">{summary ? formatHours(summary.hours) : "—"}</span>
    </div>
  </div>
{/if}

<style src="./app.css"></style>
