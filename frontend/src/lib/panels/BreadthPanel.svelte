<script lang="ts">
  import type { EChartsOption } from "echarts";
  import { chart } from "../charts";
  import { store } from "../store.svelte";

  const b = $derived(store.breadth);
  const pct = $derived(b ? Math.round(b.pct_advancing * 100) : 0);

  const option = $derived<EChartsOption>({
    series: [
      {
        type: "gauge",
        min: 0,
        max: 100,
        radius: "92%",
        progress: { show: true, width: 14 },
        axisLine: { lineStyle: { width: 14 } },
        pointer: { show: false },
        axisTick: { show: false },
        splitLine: { show: false },
        axisLabel: { distance: 18, color: "#8b949e", fontSize: 10 },
        title: { show: false },
        detail: {
          valueAnimation: true,
          formatter: "{value}%",
          color: "#e6edf3",
          fontSize: 26,
          offsetCenter: [0, 0],
        },
        data: [{ value: pct }],
      },
    ],
  });
</script>

<div class="breadth">
  <div class="chart" use:chart={option}></div>
  {#if b}
    <div class="legend">
      <span class="up">▲ {b.advancers}</span>
      <span class="muted">● {b.unchanged}</span>
      <span class="down">▼ {b.decliners}</span>
    </div>
  {/if}
</div>
