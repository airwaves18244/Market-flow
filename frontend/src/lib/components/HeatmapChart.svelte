<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { SectorRow } from "../types";

  let { sectors = [] }: { sectors: SectorRow[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function render() {
    if (!chart) return;
    // Сортируем по обороту: крупнейшие сектора слева.
    const sorted = [...sectors].sort((a, b) => b.turnover - a.turnover);
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: {
        position: "top",
        formatter: (p: any) => `${p.data[0]}<br/>изм: ${p.data[2]}%`,
      },
      grid: { left: 70, right: 12, top: 10, bottom: 60 },
      xAxis: {
        type: "category",
        data: sorted.map((s) => s.sector),
        axisLabel: { interval: 0, rotate: 45, fontSize: 10, color: "#8b949e" },
      },
      yAxis: {
        type: "category",
        data: ["Изм. %"],
        axisLabel: { color: "#8b949e", fontSize: 10 },
      },
      visualMap: {
        min: -5,
        max: 5,
        calculable: true,
        orient: "horizontal",
        left: "center",
        bottom: 0,
        inRange: { color: ["#ef5350", "#30363d", "#26a69a"] },
        textStyle: { color: "#8b949e", fontSize: 10 },
      },
      series: [
        {
          name: "Изм. %",
          type: "heatmap",
          data: sorted.map((s) => [
            s.sector,
            "Изм. %",
            Number((s.weightedChange * 100).toFixed(2)),
          ]),
          itemStyle: { borderColor: "#0d1117", borderWidth: 1 },
        },
      ],
    });
  }

  $effect(() => {
    void sectors;
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

<div class="heatmap" bind:this={el}></div>

<style>
  .heatmap {
    width: 100%;
    height: 100%;
    min-height: 220px;
  }
</style>
