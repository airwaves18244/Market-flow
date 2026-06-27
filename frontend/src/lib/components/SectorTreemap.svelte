<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { SectorRow } from "../types";

  let { rows = [] }: { rows: SectorRow[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  // Цвет плитки: зелёный — рост, красный — падение (насыщенность ~ модуль).
  function colorFor(change: number): string {
    const norm = Math.max(-0.03, Math.min(0.03, change)) / 0.03;
    const a = 0.25 + 0.65 * Math.abs(norm);
    return norm >= 0 ? `rgba(38,166,154,${a})` : `rgba(239,83,80,${a})`;
  }

  function render() {
    if (!chart) return;
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: {
        formatter: (p: any) =>
          `${p.name}<br/>оборот: ${Math.round(p.value).toLocaleString("ru-RU")}`,
      },
      series: [
        {
          type: "treemap",
          roam: false,
          nodeClick: false,
          breadcrumb: { show: false },
          label: { show: true, formatter: "{b}", color: "#e6edf3", fontSize: 12 },
          itemStyle: { borderColor: "#0d1117", borderWidth: 2, gapWidth: 2 },
          data: rows.map((r) => ({
            name: r.sector,
            value: r.turnover,
            itemStyle: { color: colorFor(r.weightedChange) },
          })),
        },
      ],
    });
  }

  $effect(() => {
    void rows;
    render();
  });

  onMount(() => {
    chart = echarts.init(el);
    render();
    ro = new ResizeObserver(() => chart?.resize());
    ro.observe(el);
  });

  onDestroy(() => {
    ro?.disconnect();
    chart?.dispose();
  });
</script>

<div class="treemap" bind:this={el}></div>

<style>
  .treemap {
    width: 100%;
    height: 100%;
    min-height: 240px;
  }
</style>
