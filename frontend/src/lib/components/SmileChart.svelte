<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { SmileFitDto, SmilePointInput } from "../types";

  // Улыбка волатильности: рыночные точки (scatter, размер ~ вес/OI) + кривая
  // подгонки выбранной модели.
  let {
    points = [],
    fit = null,
  }: { points: SmilePointInput[]; fit: SmileFitDto | null } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function render() {
    if (!chart) return;
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: { trigger: "item" },
      grid: { left: 48, right: 16, top: 24, bottom: 32 },
      xAxis: {
        type: "value",
        name: "страйк",
        scale: true,
        axisLine: { lineStyle: { color: "#3a4553" } },
        splitLine: { lineStyle: { color: "#1d2530" } },
      },
      yAxis: {
        type: "value",
        name: "IV",
        scale: true,
        axisLabel: { formatter: (v: number) => (v * 100).toFixed(0) + "%" },
        axisLine: { lineStyle: { color: "#3a4553" } },
        splitLine: { lineStyle: { color: "#1d2530" } },
      },
      series: [
        {
          name: "рынок",
          type: "scatter",
          symbolSize: (d: number[]) => 6 + 12 * (d[2] ?? 1),
          itemStyle: { color: "#4f9cf9" },
          data: points.map((p) => [p.strike, p.iv, p.weight ?? 1]),
        },
        {
          name: fit ? fit.model : "модель",
          type: "line",
          smooth: true,
          showSymbol: false,
          lineStyle: { color: "#f5a623", width: 2 },
          data: (fit?.curve ?? []).map((c) => [c.strike, c.iv]),
        },
      ],
    });
  }

  $effect(() => {
    void points;
    void fit;
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

<div class="smile" bind:this={el}></div>

<style>
  .smile {
    width: 100%;
    height: 100%;
    min-height: 260px;
  }
</style>
