<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";
  import type { FlowEdgeDto } from "../types";
  import { assetColor, assetLabel } from "../assetClass";

  let { edges = [] }: { edges: FlowEdgeDto[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  // Sankey запрещает циклы и требует, чтобы узлы источника и приёмника
  // отличались — разделяем имена суффиксом направления.
  function render() {
    if (!chart) return;
    const nodeSet = new Set<string>();
    for (const e of edges) {
      nodeSet.add(`${e.from} ▸`);
      nodeSet.add(`▸ ${e.to}`);
    }
    const nodes = [...nodeSet].map((name) => {
      const code = name.replace(/[▸ ]/g, "");
      return { name, itemStyle: { color: assetColor(code) } };
    });
    const links = edges.map((e) => ({
      source: `${e.from} ▸`,
      target: `▸ ${e.to}`,
      value: Number((e.weight * 100).toFixed(2)),
    }));

    chart.setOption({
      backgroundColor: "transparent",
      tooltip: {
        trigger: "item",
        triggerOn: "mousemove",
        formatter: (p: any) =>
          p.dataType === "edge" ? `переток доли: ${p.data.value}%` : p.name,
      },
      series: [
        {
          type: "sankey",
          left: 8,
          right: 90,
          top: 10,
          bottom: 10,
          nodeWidth: 14,
          nodeGap: 10,
          label: {
            color: "#e6edf3",
            fontSize: 11,
            formatter: (p: any) => assetLabel(p.name.replace(/[▸ ]/g, "")),
          },
          lineStyle: { color: "gradient", opacity: 0.4 },
          data: nodes,
          links,
        },
      ],
    });
  }

  $effect(() => {
    void edges;
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

<div class="wrap">
  <div class="sankey" bind:this={el}></div>
  {#if edges.length === 0}
    <div class="empty">Нет значимых перетоков между классами за период</div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 220px;
  }
  .sankey {
    width: 100%;
    height: 100%;
    min-height: 220px;
  }
  .empty {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
    font-size: 12px;
    text-align: center;
  }
</style>
