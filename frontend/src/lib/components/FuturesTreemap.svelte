<script lang="ts">
  import { onMount } from "svelte";
  import * as echarts from "echarts";
  import type { FutureGroupDto } from "../types";

  export let futures: FutureGroupDto[];

  let container: HTMLDivElement;

  onMount(() => {
    if (!container) return;

    const chart = echarts.init(container);

    const data = futures.map((f) => ({
      name: f.group,
      value: f.turnover,
      itemStyle: {
        color:
          f.weightedChange > 0
            ? `rgba(38, 166, 154, ${Math.min(1, 0.5 + f.weightedChange * 10)})`
            : `rgba(239, 83, 80, ${Math.min(1, 0.5 + Math.abs(f.weightedChange) * 10)})`,
      },
      label: {
        show: true,
        formatter: `{b}\n${f.contracts} контрактов`,
      },
    }));

    const option = {
      tooltip: {
        trigger: "item" as const,
        formatter: (params: {
          name: string;
          value: number;
          data: {
            label: { show: boolean; formatter: string };
          };
        }) => {
          const f = futures.find((x) => x.group === params.name)!;
          return `
            ${params.name}<br/>
            Turnover: ${(params.value / 1_000_000).toFixed(1)}M<br/>
            Contracts: ${f.contracts}<br/>
            Change: ${(f.weightedChange * 100).toFixed(2)}%
          `;
        },
      },
      series: [
        {
          type: "treemap" as const,
          label: { show: true, fontSize: 12, fontWeight: "bold" },
          breadcrumb: { show: false },
          data: data,
        },
      ],
    };

    chart.setOption(option);

    return () => chart.dispose();
  });
</script>

<div class="futures-treemap">
  <div bind:this={container} style="height: 250px;"></div>
</div>

<style>
  .futures-treemap {
    padding: 12px;
    background: var(--bg-secondary, #161b22);
    border-radius: 6px;
    border: 1px solid var(--border, #30363d);
  }
</style>
