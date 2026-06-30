<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    createChart,
    ColorType,
    type IChartApi,
    type ISeriesApi,
    type UTCTimestamp,
    type SeriesMarker,
    type Time,
  } from "lightweight-charts";
  import type { BarPoint, FootprintBarDto, RobotSignalDto } from "../types";

  let {
    bars = [],
    footprint = [],
    signals = [],
  }: {
    bars: BarPoint[];
    footprint: FootprintBarDto[];
    signals: RobotSignalDto[];
  } = $props();

  let el: HTMLDivElement;
  let chart: IChartApi | undefined;
  let candles: ISeriesApi<"Candlestick"> | undefined;
  let deltaSeries: ISeriesApi<"Histogram"> | undefined;
  let cumSeries: ISeriesApi<"Line"> | undefined;

  const markerColor: Record<string, string> = {
    same_lot: "#f5b942",
    iceberg: "#4f9cf9",
    absorption: "#ef5350",
  };
  const markerLabel: Record<string, string> = {
    same_lot: "SL",
    iceberg: "IB",
    absorption: "AB",
  };

  function render() {
    if (!candles || !deltaSeries || !cumSeries) return;
    candles.setData(
      bars.map((b) => ({
        time: b.ts as UTCTimestamp,
        open: b.open,
        high: b.high,
        low: b.low,
        close: b.close,
      })),
    );
    deltaSeries.setData(
      footprint.map((f) => ({
        time: f.ts as UTCTimestamp,
        value: f.delta,
        color: f.delta >= 0 ? "rgba(38, 166, 154, 0.6)" : "rgba(239, 83, 80, 0.6)",
      })),
    );
    cumSeries.setData(
      footprint.map((f) => ({ time: f.ts as UTCTimestamp, value: f.cumulativeDelta })),
    );

    const markers: SeriesMarker<Time>[] = signals.map((s) => ({
      time: s.ts as UTCTimestamp,
      position: "aboveBar",
      color: markerColor[s.kind] ?? "#8b98a9",
      shape: "circle",
      text: markerLabel[s.kind] ?? "?",
    }));
    markers.sort((a, b) => (a.time as number) - (b.time as number));
    candles.setMarkers(markers);
    chart?.timeScale().fitContent();
  }

  $effect(() => {
    void bars;
    void footprint;
    void signals;
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
    candles = chart.addCandlestickSeries({
      upColor: "#26a69a",
      downColor: "#ef5350",
      borderVisible: false,
      wickUpColor: "#26a69a",
      wickDownColor: "#ef5350",
    });
    // Дельта-гистограмма и накопленная дельта — в нижней зоне (отдельная шкала).
    deltaSeries = chart.addHistogramSeries({ priceScaleId: "delta", priceLineVisible: false });
    cumSeries = chart.addLineSeries({
      priceScaleId: "delta",
      color: "#c792ea",
      lineWidth: 1,
      priceLineVisible: false,
    });
    chart.priceScale("delta").applyOptions({ scaleMargins: { top: 0.75, bottom: 0 } });
    render();
  });

  onDestroy(() => chart?.remove());
</script>

<div class="delta-chart" bind:this={el}></div>

<style>
  .delta-chart {
    width: 100%;
    height: 100%;
    min-height: 320px;
  }
</style>
