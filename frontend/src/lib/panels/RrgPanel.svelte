<script lang="ts">
  import type { EChartsOption } from "echarts";
  import { chart } from "../charts";
  import { store } from "../store.svelte";

  // RRG: x = RS-Ratio, y = RS-Momentum, центр (100, 100) делит на 4 квадранта:
  // лидеры (П-В), слабеющие (П-Н), отстающие (Л-Н), улучшающиеся (Л-В).
  const points = $derived(store.rrg);

  const option = $derived<EChartsOption>({
    tooltip: {
      formatter: (p: unknown) => {
        const item = p as { name: string; value: [number, number] };
        return `${item.name}<br/>RS-Ratio: ${item.value[0]}<br/>RS-Mom: ${item.value[1]}`;
      },
    },
    grid: { left: 48, right: 16, top: 16, bottom: 32 },
    xAxis: { name: "RS-Ratio", type: "value", scale: true, splitLine: { show: false } },
    yAxis: { name: "RS-Mom", type: "value", scale: true, splitLine: { show: false } },
    series: [
      {
        type: "scatter",
        symbolSize: 14,
        label: { show: true, position: "right", formatter: "{b}", color: "#e6edf3", fontSize: 10 },
        itemStyle: {
          color: (p: unknown) => {
            const v = (p as { value: [number, number] }).value;
            const right = v[0] >= 100;
            const up = v[1] >= 100;
            if (right && up) return "#3fb950"; // лидеры
            if (right && !up) return "#d29922"; // слабеющие
            if (!right && up) return "#58a6ff"; // улучшающиеся
            return "#f85149"; // отстающие
          },
        },
        data: points.map((p) => ({ name: p.sector, value: [p.rs_ratio, p.rs_momentum] })),
        markLine: {
          silent: true,
          symbol: "none",
          lineStyle: { color: "#484f58", type: "dashed" },
          data: [{ xAxis: 100 }, { yAxis: 100 }],
        },
      },
    ],
  });
</script>

<div class="chart" use:chart={option}></div>
