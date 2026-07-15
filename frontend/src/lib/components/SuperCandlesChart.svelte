<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    createChart,
    ColorType,
    type IChartApi,
    type ISeriesApi,
    type UTCTimestamp,
  } from "lightweight-charts";
  import type { TradestatsDto } from "../types";

  // Супер-свечи: свечи + линия VWAP + гистограмма объёма (Фаза 10).
  // Данные — датасет ALGOPACK `tradestats` (боевой IPC/мок, T11).
  let { bars = [] }: { bars: TradestatsDto[] } = $props();

  let el: HTMLDivElement;
  let chart: IChartApi | undefined;
  let cs: ISeriesApi<"Candlestick"> | undefined;
  let vwap: ISeriesApi<"Line"> | undefined;
  let vol: ISeriesApi<"Histogram"> | undefined;

  function render() {
    if (!cs || !vwap || !vol) return;
    cs.setData(
      bars.map((b) => ({
        time: b.ts as UTCTimestamp,
        open: b.prOpen,
        high: b.prHigh,
        low: b.prLow,
        close: b.prClose,
      })),
    );
    vwap.setData(bars.map((b) => ({ time: b.ts as UTCTimestamp, value: b.prVwap })));
    vol.setData(
      bars.map((b) => ({
        time: b.ts as UTCTimestamp,
        value: b.vol,
        color: b.prClose >= b.prOpen ? "rgba(38,166,154,.5)" : "rgba(239,83,80,.5)",
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
      layout: { background: { type: ColorType.Solid, color: "transparent" }, textColor: "#8b98a9" },
      grid: { vertLines: { color: "#161d27" }, horzLines: { color: "#161d27" } },
      rightPriceScale: { borderColor: "#1d2530" },
      timeScale: { borderColor: "#1d2530", timeVisible: true },
    });
    cs = chart.addCandlestickSeries({
      upColor: "#26a69a",
      downColor: "#ef5350",
      borderVisible: false,
      wickUpColor: "#26a69a",
      wickDownColor: "#ef5350",
    });
    vwap = chart.addLineSeries({
      color: "#f5a623",
      lineWidth: 1,
      priceLineVisible: false,
      lastValueVisible: false,
    });
    vol = chart.addHistogramSeries({ priceScaleId: "vol", priceFormat: { type: "volume" } });
    chart.priceScale("vol").applyOptions({ scaleMargins: { top: 0.82, bottom: 0 } });
    render();
  });

  onDestroy(() => chart?.remove());
</script>

<div class="sc" bind:this={el}></div>

<style>
  .sc {
    width: 100%;
    height: 100%;
    min-height: 240px;
  }
</style>
