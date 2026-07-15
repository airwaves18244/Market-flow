<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { TradestatsDto } from "../types";

  // Дисбаланс покупок/продаж по барам (Фаза 10): зелёный вверх / красный вниз.
  // Данные — датасет ALGOPACK `tradestats` (боевой IPC/мок, T11).
  let { bars = [] }: { bars: TradestatsDto[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function render() {
    if (!chart) return;
    chart.setOption({
      backgroundColor: "transparent",
      grid: { left: 44, right: 12, top: 6, bottom: 16 },
      xAxis: {
        type: "category",
        data: bars.map((_, i) => i),
        axisLabel: { show: false },
        axisLine: { lineStyle: { color: "#1d2530" } },
      },
      yAxis: {
        min: -1,
        max: 1,
        splitNumber: 2,
        axisLabel: { color: "#8b98a9", fontSize: 10 },
        axisLine: { lineStyle: { color: "#1d2530" } },
        splitLine: { lineStyle: { color: "#161d27" } },
      },
      series: [
        {
          type: "bar",
          data: bars.map((b) => ({
            value: +b.disb.toFixed(2),
            itemStyle: { color: b.disb >= 0 ? "#26a69a" : "#ef5350" },
          })),
        },
      ],
    });
  }

  $effect(() => {
    void bars;
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

<div class="disb" bind:this={el}></div>

<style>
  .disb {
    width: 100%;
    height: 70px;
  }
</style>
