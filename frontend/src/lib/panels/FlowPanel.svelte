<script lang="ts">
  import type { EChartsOption } from "echarts";
  import type { FlowPoint } from "../types";
  import { chart } from "../charts";

  let { flow, symbol }: { flow: FlowPoint[]; symbol: string } = $props();

  const option = $derived<EChartsOption>({
    tooltip: { trigger: "axis" },
    grid: { left: 56, right: 16, top: 24, bottom: 28 },
    xAxis: { type: "time" },
    yAxis: { type: "value", scale: true },
    series: [
      {
        type: "line",
        name: "нетто-поток",
        showSymbol: false,
        areaStyle: {},
        data: flow.map((p) => [p.ts * 1000, p.net_flow]),
      },
    ],
  });
</script>

<section class="panel">
  <h2>Нетто-поток · {symbol}</h2>
  <div class="chart" use:chart={option}></div>
</section>
