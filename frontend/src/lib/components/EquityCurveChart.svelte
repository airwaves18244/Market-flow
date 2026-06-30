<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    createChart,
    ColorType,
    type IChartApi,
    type ISeriesApi,
    type UTCTimestamp,
  } from "lightweight-charts";
  import type { EquityPointDto } from "../types";

  let { points = [] }: { points: EquityPointDto[] } = $props();

  let el: HTMLDivElement;
  let chart: IChartApi | undefined;
  let series: ISeriesApi<"Area"> | undefined;

  function render() {
    if (!series) return;
    series.setData(points.map((p) => ({ time: p.ts as UTCTimestamp, value: p.equity })));
    chart?.timeScale().fitContent();
  }

  $effect(() => {
    void points;
    render();
  });

  onMount(() => {
    chart = createChart(el, {
      autoSize: true,
      layout: {
        background: { type: ColorType.Solid, color: "transparent" },
        textColor: "#8b98a9",
      },
      grid: {
        vertLines: { color: "#1d2530" },
        horzLines: { color: "#1d2530" },
      },
      rightPriceScale: { borderColor: "#1d2530" },
      timeScale: { borderColor: "#1d2530" },
    });
    series = chart.addAreaSeries({
      lineColor: "#4f9cf9",
      topColor: "rgba(79, 156, 249, 0.3)",
      bottomColor: "rgba(79, 156, 249, 0.02)",
      lineWidth: 2,
    });
    render();
  });

  onDestroy(() => chart?.remove());
</script>

<div class="equity" bind:this={el}></div>

<style>
  .equity {
    width: 100%;
    height: 100%;
    min-height: 240px;
  }
</style>
