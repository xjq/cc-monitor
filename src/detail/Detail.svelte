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
  onDestroy(() => { if (refreshTimer) window.clearTimeout(refreshTimer); unlisten?.(); plot?.destroy(); });
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
