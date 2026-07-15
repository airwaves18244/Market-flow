<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { FutoiDto } from "../types";

  // FUTOI: открытые позиции физлиц/юрлиц (Фаза 10). Режим long/short/net
  // выбирает, как агрегировать длинные и короткие позиции. Данные — датасет
  // ALGOPACK `futoi` (боевой IPC/мок, T11): точки по (ts, clgroup), группировка
  // в две временные серии — здесь, в компоненте.
  let {
    points = [],
    mode = "net",
  }: {
    points: FutoiDto[];
    mode: "long" | "short" | "net";
  } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function timeLabel(ts: number): string {
    const d = new Date(ts * 1000);
    return `${String(d.getUTCHours()).padStart(2, "0")}:${String(d.getUTCMinutes()).padStart(2, "0")}`;
  }

  /** Значения в тыс. контрактов (см. `yAxis.name` ниже) — `posLong`/`posShort`
   * в DTO хранятся в штуках. */
  function seriesFor(group: "fiz" | "yur"): { times: string[]; values: number[] } {
    const rows = points.filter((p) => p.clgroup === group).sort((a, b) => a.ts - b.ts);
    return {
      times: rows.map((p) => timeLabel(p.ts)),
      values: rows.map(
        (p) => (mode === "long" ? p.posLong : mode === "short" ? -p.posShort : p.net) / 1000,
      ),
    };
  }

  function render() {
    if (!chart) return;
    const fiz = seriesFor("fiz");
    const yur = seriesFor("yur");
    const times = fiz.times.length >= yur.times.length ? fiz.times : yur.times;
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
        xAxis: { type: "category", data: times, ...axis },
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
            data: fiz.values,
            lineStyle: { color: "#4f9cf9", width: 2 },
            itemStyle: { color: "#4f9cf9" },
            areaStyle: { color: "rgba(79,156,249,.08)" },
          },
          {
            name: "Юрлица",
            type: "line",
            smooth: true,
            data: yur.values,
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
    void points;
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
