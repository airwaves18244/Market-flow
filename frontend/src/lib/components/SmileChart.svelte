<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { SmileFitDto, SmilePointInput } from "../types";

  // Улыбка волатильности: рыночные точки (scatter, размер ~ вес/OI) + кривые
  // подгонки одной или нескольких моделей.
  export type SmileCurveLine = {
    name: string;
    curve: { strike: number; iv: number }[];
    color: string;
    active: boolean;
  };

  let {
    points = [],
    fit = null,
    fits = null,
  }: {
    points: SmilePointInput[];
    fit?: SmileFitDto | null;
    fits?: SmileCurveLine[] | null;
  } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function curveSeries() {
    if (fits && fits.length > 0) {
      return fits.map((f) => ({
        name: f.name,
        type: "line" as const,
        smooth: true,
        showSymbol: false,
        lineStyle: { color: f.color, width: f.active ? 3 : 1.6 },
        data: f.curve.map((c) => [c.strike, c.iv]),
      }));
    }
    return [
      {
        name: fit ? fit.model : "модель",
        type: "line" as const,
        smooth: true,
        showSymbol: false,
        lineStyle: { color: "#f5a623", width: 2 },
        data: (fit?.curve ?? []).map((c) => [c.strike, c.iv]),
      },
    ];
  }

  function render() {
    if (!chart) return;
    chart.setOption(
      {
        backgroundColor: "transparent",
        tooltip: { trigger: "item" },
        legend: { top: 0, textStyle: { color: "#8b98a9", fontSize: 11 } },
        grid: { left: 48, right: 16, top: 30, bottom: 32 },
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
            itemStyle: { color: "rgba(230,237,243,.85)" },
            data: points.map((p) => [p.strike, p.iv, p.weight ?? 1]),
          },
          ...curveSeries(),
        ],
      },
      true,
    );
  }

  $effect(() => {
    void points;
    void fit;
    void fits;
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
