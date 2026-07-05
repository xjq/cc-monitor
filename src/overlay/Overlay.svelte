<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { formatTokens, formatHours, formatCurrency } from "../lib/format";

  interface Summary {
    tokens: number;
    cost_usd: number;
    hours: number;
  }

  let summary: Summary | null = null;
  let usdToCny: number = 7.2;
  let showCostCny: boolean = false;

  let dragStartX = 0;
  let dragStartY = 0;

  async function loadSettings() {
    try {
      const settings = await invoke<{ usd_to_cny: number }>("get_settings");
      usdToCny = settings.usd_to_cny;
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function handleDbStatus(event: any) {
    summary = event.payload;
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

  function onMouseDown(e: MouseEvent) {
    dragStartX = e.screenX;
    dragStartY = e.screenY;
  }

  function onMouseUp(e: MouseEvent) {
    const dx = e.screenX - dragStartX;
    const dy = e.screenY - dragStartY;
    if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
      invoke("save_overlay_position", { x: e.screenX, y: e.screenY });
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

<div
  class="overlay"
  onmousedown={onMouseDown}
  onmouseup={onMouseUp}
>
  <div class="metric">
    <span class="metric-label">Tokens</span>
    <span class="metric-value">{summary ? formatTokens(summary.tokens) : "—"}</span>
  </div>
  <div class="metric">
    <span class="metric-label">Cost</span>
    <span
      class="metric-value clickable"
      onclick={openDetailWindow}
      title="Click to open detail window"
    >
      {#if summary}
        {#if showCostCny}
          {formatCurrency(summary.cost_usd, "CNY", usdToCny)}
        {:else}
          {formatCurrency(summary.cost_usd, "USD")}
        {/if}
      {:else}
        —
      {/if}
    </span>
  </div>
  <div class="metric">
    <span class="metric-label">Hours</span>
    <span class="metric-value">{summary ? formatHours(summary.hours) : "—"}</span>
  </div>
</div>

<style src="./app.css"></style>
