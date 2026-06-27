<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    createChart,
    ColorType,
    type IChartApi,
    type ISeriesApi,
    type UTCTimestamp,
  } from "lightweight-charts";
  import type { BarPoint } from "../types";

  let { bars = [] }: { bars: BarPoint[] } = $props();

  let el: HTMLDivElement;
  let chart: IChartApi | undefined;
  let series: ISeriesApi<"Candlestick"> | undefined;

  function render() {
    if (!series) return;
    series.setData(
      bars.map((b) => ({
        time: b.ts as UTCTimestamp,
        open: b.open,
        high: b.high,
        low: b.low,
        close: b.close,
      })),
    );
    chart?.timeScale().fitContent();
  }

  $effect(() => {
    void bars;
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
    series = chart.addCandlestickSeries({
      upColor: "#26a69a",
      downColor: "#ef5350",
      borderVisible: false,
      wickUpColor: "#26a69a",
      wickDownColor: "#ef5350",
    });
    render();
  });

  onDestroy(() => chart?.remove());
</script>

<div class="candles" bind:this={el}></div>

<style>
  .candles {
    width: 100%;
    height: 100%;
    min-height: 240px;
  }
</style>
