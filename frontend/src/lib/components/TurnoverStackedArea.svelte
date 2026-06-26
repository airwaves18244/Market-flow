<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { TurnoverByClassPoint } from "../types";
  import { assetColor, assetLabel } from "../assetClass";

  let { points = [] }: { points: TurnoverByClassPoint[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function fmtDate(ts: number): string {
    return new Date(ts * 1000).toLocaleDateString("ru-RU", {
      day: "2-digit",
      month: "2-digit",
    });
  }

  function series(code: "equity" | "future" | "bond") {
    return {
      name: assetLabel(code),
      type: "line" as const,
      stack: "total",
      areaStyle: { color: assetColor(code), opacity: 0.5 },
      lineStyle: { width: 1, color: assetColor(code) },
      itemStyle: { color: assetColor(code) },
      showSymbol: false,
      data: points.map((p) => Math.round(p[code])),
    };
  }

  function render() {
    if (!chart) return;
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: { trigger: "axis" },
      legend: { top: 0, textStyle: { color: "#8b949e", fontSize: 11 } },
      grid: { left: 55, right: 16, top: 30, bottom: 28 },
      xAxis: {
        type: "category",
        boundaryGap: false,
        data: points.map((p) => fmtDate(p.ts)),
        axisLabel: { color: "#8b949e", fontSize: 10 },
      },
      yAxis: {
        type: "value",
        axisLabel: {
          color: "#8b949e",
          fontSize: 10,
          formatter: (v: number) => `${(v / 1e6).toFixed(0)}M`,
        },
        splitLine: { lineStyle: { color: "#21262d" } },
      },
      series: [series("equity"), series("future"), series("bond")],
    });
  }

  $effect(() => {
    void points;
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

<div class="area" bind:this={el}></div>

<style>
  .area {
    width: 100%;
    height: 100%;
    min-height: 220px;
  }
</style>
