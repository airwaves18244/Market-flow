<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { Hi2Dto } from "../types";

  // Концентрация рынка HI2 во времени (Фаза 10) с зонами «умеренная» /
  // «доминирование» и пороговыми линиями. Данные — датасет ALGOPACK `hi2`
  // (боевой IPC/мок, T11).
  let { points = [] }: { points: Hi2Dto[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function timeLabel(ts: number): string {
    const d = new Date(ts * 1000);
    return `${String(d.getUTCHours()).padStart(2, "0")}:${String(d.getUTCMinutes()).padStart(2, "0")}`;
  }

  function render() {
    if (!chart) return;
    const sorted = [...points].sort((a, b) => a.ts - b.ts);
    const times = sorted.map((p) => timeLabel(p.ts));
    const vals = sorted.map((p) => p.concentration);
    const axis = {
      axisLine: { lineStyle: { color: "#1d2530" } },
      axisLabel: { color: "#8b98a9", fontSize: 10 },
      splitLine: { lineStyle: { color: "#161d27" } },
    };
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        backgroundColor: "#1a232e",
        borderColor: "#1d2530",
        textStyle: { color: "#e6edf3", fontSize: 11 },
      },
      grid: { left: 46, right: 14, top: 14, bottom: 24 },
      xAxis: {
        type: "category",
        data: times,
        ...axis,
        axisLabel: { color: "#8b98a9", fontSize: 10, interval: 5 },
      },
      yAxis: { ...axis, min: 0, max: 0.5 },
      series: [
        {
          type: "line",
          data: vals,
          smooth: true,
          symbol: "none",
          lineStyle: { color: "#4f9cf9", width: 2 },
          areaStyle: { color: "rgba(79,156,249,.06)" },
          markArea: {
            silent: true,
            data: [
              [{ yAxis: 0, itemStyle: { color: "rgba(38,166,154,.05)" } }, { yAxis: 0.18 }],
              [{ yAxis: 0.3, itemStyle: { color: "rgba(239,83,80,.06)" } }, { yAxis: 0.5 }],
            ],
          },
          markLine: {
            silent: true,
            symbol: "none",
            lineStyle: { color: "#8b98a9", type: "dashed" },
            data: [
              { yAxis: 0.18, label: { formatter: "умеренная", color: "#8b98a9", fontSize: 9 } },
              {
                yAxis: 0.3,
                label: { formatter: "доминирование", color: "#ef5350", fontSize: 9 },
              },
            ],
          },
        },
      ],
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

<div class="hi2" bind:this={el}></div>

<style>
  .hi2 {
    width: 100%;
    height: 100%;
    min-height: 280px;
  }
</style>
