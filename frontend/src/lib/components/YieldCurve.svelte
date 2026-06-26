<script lang="ts">
  import { onMount } from "svelte";
  import * as echarts from "echarts";
  import type { YieldCurvePoint } from "../types";

  export let curve: YieldCurvePoint[];

  let container: HTMLDivElement;

  onMount(() => {
    if (!container) return;

    const chart = echarts.init(container);

    const maturities = curve.map((c) => `${c.maturityYears}y`);
    const yields = curve.map((c) => c.yieldPct);

    const option = {
      tooltip: { trigger: "axis" as const },
      grid: { left: 50, right: 20, top: 20, bottom: 30 },
      xAxis: {
        type: "category" as const,
        data: maturities,
        axisLabel: { fontSize: 10 },
      },
      yAxis: {
        type: "value" as const,
        name: "Yield %",
        axisLabel: { fontSize: 10 },
      },
      series: [
        {
          name: "Yield",
          type: "line" as const,
          data: yields,
          smooth: true,
          areaStyle: {
            color: "rgba(79, 156, 249, 0.3)",
          },
          lineStyle: {
            color: "rgba(79, 156, 249, 0.8)",
            width: 2,
          },
          itemStyle: {
            color: "#4f9cf9",
          },
        },
      ],
    };

    chart.setOption(option);

    return () => chart.dispose();
  });
</script>

<div class="yield-curve">
  <h3>Bond Yield Curve</h3>
  <div bind:this={container} style="height: 200px;"></div>
</div>

<style>
  .yield-curve {
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
