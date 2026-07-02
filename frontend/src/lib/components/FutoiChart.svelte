<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { FutoiSeries } from "../algoMock";

  // FUTOI: открытые позиции физлиц/юрлиц (Фаза 10). Режим long/short/net
  // выбирает, как агрегировать длинные и короткие позиции.
  let {
    series,
    mode = "net",
  }: {
    series: FutoiSeries;
    mode: "long" | "short" | "net";
  } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  const pick = (L: number[], S: number[]) =>
    mode === "long" ? L : mode === "short" ? S.map((x) => -x) : L.map((x, i) => x - S[i]);

  function render() {
    if (!chart) return;
    const axis = {
      axisLine: { lineStyle: { color: "#1d2530" } },
      axisLabel: { color: "#8b98a9", fontSize: 10 },
      splitLine: { lineStyle: { color: "#161d27" } },
    };
    chart.setOption(
      {
        backgroundColor: "transparent",
        tooltip: {
          trigger: "axis",
          backgroundColor: "#1a232e",
          borderColor: "#1d2530",
          textStyle: { color: "#e6edf3", fontSize: 11 },
        },
        legend: { data: ["Физлица", "Юрлица"], textStyle: { color: "#8b98a9", fontSize: 11 }, top: 0 },
        grid: { left: 52, right: 14, top: 28, bottom: 24 },
        xAxis: { type: "category", data: series.times, ...axis },
        yAxis: {
          ...axis,
          name: "тыс. контрактов",
          nameTextStyle: { color: "#8b98a9", fontSize: 10 },
        },
        series: [
          {
            name: "Физлица",
            type: "line",
            smooth: true,
            data: pick(series.fizL, series.fizS),
            lineStyle: { color: "#4f9cf9", width: 2 },
            itemStyle: { color: "#4f9cf9" },
            areaStyle: { color: "rgba(79,156,249,.08)" },
          },
          {
            name: "Юрлица",
            type: "line",
            smooth: true,
            data: pick(series.yurL, series.yurS),
            lineStyle: { color: "#f5a623", width: 2 },
            itemStyle: { color: "#f5a623" },
            areaStyle: { color: "rgba(245,166,35,.08)" },
          },
        ],
      },
      true,
    );
  }

  $effect(() => {
    void series;
    void mode;
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

<div class="futoi" bind:this={el}></div>

<style>
  .futoi {
    width: 100%;
    height: 100%;
    min-height: 300px;
  }
</style>
