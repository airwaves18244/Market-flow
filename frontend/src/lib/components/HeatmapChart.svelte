<script lang="ts">
  import { onMount } from "svelte";
  import * as echarts from "echarts";
  import type { SectorRow } from "../types";

  export let sectors: SectorRow[];

  let container: HTMLDivElement;

  onMount(() => {
    if (!container) return;

    const chart = echarts.init(container);

    // Sort by turnover for better visualization
    const sorted = [...sectors].sort((a, b) => b.turnover - a.turnover);

    // Convert to heatmap-friendly format: columns are sectors, rows can be metrics
    const data = sorted.map((s, i) => {
      const changePercent = (s.weightedChange * 100).toFixed(1);
      return [i, 0, changePercent]; // x=index, y=metric(0), value=change%
    });

    const option = {
      tooltip: { trigger: "item" as const },
      grid: { left: 60, right: 10, top: 30, bottom: 30 },
      xAxis: {
        type: "category" as const,
        data: sorted.map((s) => s.sector),
        axisLabel: { interval: 0, rotate: 45, fontSize: 10 },
      },
      yAxis: {
        type: "category" as const,
        data: ["Change %"],
      },
      visualMap: {
        min: -5,
        max: 5,
        realtime: true,
        inRange: {
          color: ["#ef5350", "#f5f5f5", "#26a69a"],
        },
        textStyle: { color: "#8b949e", fontSize: 10 },
      },
      series: [
        {
          name: "Change %",
          type: "heatmap" as const,
          data: data.map((d, i) => [sorted[i].sector, "Change %", (sorted[i].weightedChange * 100).toFixed(2)]),
          itemStyle: { borderColor: "#30363d", borderWidth: 0.5 },
        },
      ],
    };

    chart.setOption(option);

    return () => chart.dispose();
  });
</script>

<div class="heatmap-chart">
  <h3>Sector Heatmap (% Change)</h3>
  <div bind:this={container} style="height: 200px;"></div>
</div>

<style>
  .heatmap-chart {
    padding: 12px;
    background: var(--bg-secondary, #161b22);
    border-radius: 6px;
    border: 1px solid var(--border, #30363d);
  }

  h3 {
    margin: 0 0 12px 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #c9d1d9);
  }
</style>
