<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { AssetClassShareDto } from "../types";
  import { assetColor, assetLabel } from "../assetClass";

  let { shares = [] }: { shares: AssetClassShareDto[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  function render() {
    if (!chart) return;
    chart.setOption({
      backgroundColor: "transparent",
      tooltip: {
        trigger: "item",
        formatter: (p: any) => `${p.name}<br/>${(p.percent ?? 0).toFixed(1)}%`,
      },
      legend: {
        bottom: 0,
        textStyle: { color: "#8b949e", fontSize: 11 },
      },
      series: [
        {
          type: "pie",
          radius: ["45%", "70%"],
          center: ["50%", "45%"],
          avoidLabelOverlap: false,
          itemStyle: { borderColor: "#0d1117", borderWidth: 2 },
          label: { show: false },
          data: shares
            .filter((s) => s.turnover > 0)
            .map((s) => ({
              name: assetLabel(s.assetClass),
              value: s.turnover,
              itemStyle: { color: assetColor(s.assetClass) },
            })),
        },
      ],
    });
  }

  $effect(() => {
    void shares;
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

<div class="donut" bind:this={el}></div>

<style>
  .donut {
    width: 100%;
    height: 100%;
    min-height: 220px;
  }
</style>
