<script lang="ts">
  import { onMount } from "svelte";
  import * as echarts from "echarts";
  import type { RrgSectorDto } from "../types";

  export let sectors: RrgSectorDto[];

  let container: HTMLDivElement;

  onMount(() => {
    if (!container) return;

    const chart = echarts.init(container);

    // RRG: scatter plot with RS-Ratio (x) vs RS-Momentum (y)
    const data = sectors.map((s) => ({
      name: s.sector,
      value: [s.rsRatio, s.rsMomentum],
      itemStyle: {
        color:
          s.quadrant === "leading"
            ? "#26a69a"
            : s.quadrant === "weakening"
              ? "#f5a623"
              : s.quadrant === "lagging"
                ? "#ef5350"
                : "#4f9cf9",
      },
    }));

    const option = {
      tooltip: { trigger: "item" as const },
      grid: { left: 50, right: 20, top: 20, bottom: 30 },
      xAxis: {
        type: "value" as const,
        name: "RS-Ratio",
        axisLine: { onZero: false },
        min: 80,
        max: 120,
        axisLabel: { fontSize: 10 },
      },
      yAxis: {
        type: "value" as const,
        name: "RS-Momentum",
        min: 80,
        max: 120,
        axisLabel: { fontSize: 10 },
      },
      series: [
        {
          name: "Sectors",
          type: "scatter" as const,
          symbolSize: 12,
          data: data,
          label: {
            show: true,
            formatter: (params: { name: string }) => params.name,
            fontSize: 10,
            offset: [5, 5],
          },
        },
      ],
    };

    // Add quadrant lines
    const xSplitLine = {
      show: true,
      lineStyle: {
        color: "#30363d",
        type: [5, 5],
      },
    };
    const ySplitLine = {
      show: true,
      lineStyle: {
        color: "#30363d",
        type: [5, 5],
      },
    };

    option.xAxis = { ...option.xAxis, splitLine: xSplitLine };
    option.yAxis = { ...option.yAxis, splitLine: ySplitLine };

    chart.setOption(option);

    return () => chart.dispose();
  });
</script>

<div class="rrg-chart">
  <h3>RRG: Sector Rotation</h3>
  <div bind:this={container} style="height: 220px;"></div>
  <div class="legend">
    <div class="legend-item leading">
      <div class="dot"></div>
      <span>Leading</span>
    </div>
    <div class="legend-item weakening">
      <div class="dot"></div>
      <span>Weakening</span>
    </div>
    <div class="legend-item lagging">
      <div class="dot"></div>
      <span>Lagging</span>
    </div>
    <div class="legend-item improving">
      <div class="dot"></div>
      <span>Improving</span>
    </div>
  </div>
</div>

<style>
  .rrg-chart {
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

  .legend {
    display: flex;
    gap: 12px;
    margin-top: 8px;
    flex-wrap: wrap;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .leading .dot {
    background: #26a69a;
  }

  .weakening .dot {
    background: #f5a623;
  }

  .lagging .dot {
    background: #ef5350;
  }

  .improving .dot {
    background: #4f9cf9;
  }

  .legend-item span {
    color: var(--text-secondary, #8b949e);
  }
</style>
