<script lang="ts">
  import type { EChartsOption } from "echarts";
  import { chart } from "../charts";
  import { store } from "../store.svelte";

  const option = $derived<EChartsOption>({
    title: { text: store.focus, left: 8, top: 4, textStyle: { fontSize: 12, color: "#8b949e" } },
    tooltip: { trigger: "axis" },
    grid: { left: 56, right: 16, top: 28, bottom: 28 },
    xAxis: { type: "time" },
    yAxis: { type: "value", scale: true },
    series: [
      {
        type: "line",
        name: "нетто-поток",
        showSymbol: false,
        areaStyle: {},
        data: store.flow.map((p) => [p.ts * 1000, p.net_flow]),
      },
    ],
  });
</script>

<div class="chart" use:chart={option}></div>
