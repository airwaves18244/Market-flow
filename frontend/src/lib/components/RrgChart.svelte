<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { RrgSectorDto } from "../types";

  let { sectors = [] }: { sectors: RrgSectorDto[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  const QUADRANT_COLOR: Record<RrgSectorDto["quadrant"], string> = {
    leading: "#26a69a",
    weakening: "#f5a623",
    lagging: "#ef5350",
    improving: "#4f9cf9",
  };

  function render() {
    if (!chart) return;
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: {
        trigger: "item",
        formatter: (p: any) =>
          `${p.data.name}<br/>RS-Ratio: ${p.data.value[0].toFixed(1)}<br/>RS-Mom: ${p.data.value[1].toFixed(1)}`,
      },
      grid: { left: 50, right: 20, top: 20, bottom: 40 },
      xAxis: {
        type: "value",
        name: "RS-Ratio",
        nameTextStyle: { color: "#8b949e", fontSize: 10 },
        axisLine: { onZero: false },
        min: 80,
        max: 120,
        axisLabel: { fontSize: 10, color: "#8b949e" },
        splitLine: { lineStyle: { color: "#21262d" } },
      },
      yAxis: {
        type: "value",
        name: "RS-Momentum",
        nameTextStyle: { color: "#8b949e", fontSize: 10 },
        min: 80,
        max: 120,
        axisLabel: { fontSize: 10, color: "#8b949e" },
        splitLine: { lineStyle: { color: "#21262d" } },
      },
      // Линии-разделители квадрантов в центре (100, 100).
      series: [
        {
          name: "Sectors",
          type: "scatter",
          symbolSize: 14,
          data: sectors.map((s) => ({
            name: s.sector,
            value: [s.rsRatio, s.rsMomentum],
            itemStyle: { color: QUADRANT_COLOR[s.quadrant] },
          })),
          label: {
            show: true,
            formatter: (p: any) => p.data.name,
            fontSize: 10,
            color: "#e6edf3",
            position: "right",
          },
          markLine: {
            silent: true,
            symbol: "none",
            lineStyle: { color: "#484f58", type: "dashed" },
            data: [{ xAxis: 100 }, { yAxis: 100 }],
          },
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

<div class="rrg" bind:this={el}></div>
<div class="legend">
  <span class="item"><i style="background:#26a69a"></i>Leading</span>
  <span class="item"><i style="background:#f5a623"></i>Weakening</span>
  <span class="item"><i style="background:#ef5350"></i>Lagging</span>
  <span class="item"><i style="background:#4f9cf9"></i>Improving</span>
</div>

<style>
  .rrg {
    width: 100%;
    height: 100%;
    min-height: 220px;
  }
  .legend {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
    margin-top: 6px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--text-secondary, #8b949e);
  }
  .item i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    display: inline-block;
  }
</style>
