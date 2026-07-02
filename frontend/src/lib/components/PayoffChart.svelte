<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { StrategyPayoffPoint } from "../types";

  // Диаграмма выплат: P&L на экспирацию и текущий по цене базового.
  let {
    payoff = [],
    breakevens = [],
  }: { payoff: StrategyPayoffPoint[]; breakevens: number[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function render() {
    if (!chart) return;
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: { trigger: "axis" },
      legend: { top: 0, textStyle: { color: "#8b949e", fontSize: 11 } },
      grid: { left: 56, right: 16, top: 28, bottom: 32 },
      xAxis: {
        type: "value",
        name: "цена",
        scale: true,
        axisLine: { lineStyle: { color: "#3a4553" } },
        splitLine: { lineStyle: { color: "#1d2530" } },
      },
      yAxis: {
        type: "value",
        name: "P&L",
        axisLine: { lineStyle: { color: "#3a4553" } },
        splitLine: { lineStyle: { color: "#1d2530" } },
      },
      series: [
        {
          name: "на экспирацию",
          type: "line",
          showSymbol: false,
          lineStyle: { color: "#4f9cf9", width: 2 },
          data: payoff.map((p) => [p.price, p.pnlExpiry]),
          markLine: {
            silent: true,
            symbol: "none",
            lineStyle: { color: "#8b949e", type: "dashed" },
            data: [{ yAxis: 0 }, ...breakevens.map((b) => ({ xAxis: b }))],
          },
        },
        {
          name: "текущий",
          type: "line",
          smooth: true,
          showSymbol: false,
          lineStyle: { color: "#f5a623", width: 1.5, type: "dashed" },
          data: payoff.map((p) => [p.price, p.pnlNow]),
        },
      ],
    });
  }

  $effect(() => {
    void payoff;
    void breakevens;
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

<div class="payoff" bind:this={el}></div>

<style>
  .payoff {
    width: 100%;
    height: 100%;
    min-height: 260px;
  }
</style>
