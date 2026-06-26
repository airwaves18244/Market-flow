<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { YieldCurvePoint } from "../types";

  let { curve = [] }: { curve: YieldCurvePoint[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function render() {
    if (!chart) return;
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        formatter: (p: any) => `${p[0].axisValue}<br/>доходность: ${p[0].data}%`,
      },
      grid: { left: 50, right: 20, top: 20, bottom: 30 },
      xAxis: {
        type: "category",
        data: curve.map((c) => `${c.maturityYears}л`),
        axisLabel: { fontSize: 10, color: "#8b949e" },
      },
      yAxis: {
        type: "value",
        name: "Доходность %",
        nameTextStyle: { color: "#8b949e", fontSize: 10 },
        axisLabel: { fontSize: 10, color: "#8b949e" },
        splitLine: { lineStyle: { color: "#21262d" } },
      },
      series: [
        {
          name: "Доходность",
          type: "line",
          smooth: true,
          data: curve.map((c) => c.yieldPct),
          areaStyle: { color: "rgba(79,156,249,0.25)" },
          lineStyle: { color: "#4f9cf9", width: 2 },
          itemStyle: { color: "#4f9cf9" },
        },
      ],
    });
  }

  $effect(() => {
    void curve;
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

<div class="yield" bind:this={el}></div>

<style>
  .yield {
    width: 100%;
    height: 100%;
    min-height: 200px;
  }
</style>
