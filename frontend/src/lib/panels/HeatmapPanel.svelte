<script lang="ts">
  import type { EChartsOption } from "echarts";
  import { chart } from "../charts";
  import { store } from "../store.svelte";

  // Хитмэп секторов: цвет = интенсивность нетто-потока (нетто-поток / оборот, %).
  const sectors = $derived(store.equity?.sectors ?? []);
  const ratios = $derived(
    sectors.map((s) => (s.turnover > 0 ? (s.net_flow / s.turnover) * 100 : 0)),
  );
  const bound = $derived(Math.max(1, ...ratios.map((r) => Math.abs(r))));

  const option = $derived<EChartsOption>({
    tooltip: {
      formatter: (p: unknown) => {
        const item = p as { data: [number, number, number]; name: string };
        return `${sectors[item.data[0]]?.sector ?? ""}<br/>поток/оборот: ${item.data[2].toFixed(2)}%`;
      },
    },
    grid: { left: 8, right: 8, top: 8, bottom: 56, containLabel: true },
    xAxis: { type: "category", data: sectors.map((s) => s.sector), axisLabel: { interval: 0, rotate: 30 } },
    yAxis: { type: "category", data: ["нетто-поток"] },
    visualMap: {
      min: -bound,
      max: bound,
      calculable: true,
      orient: "horizontal",
      left: "center",
      bottom: 0,
      inRange: { color: ["#c62828", "#3a3f47", "#2e7d32"] },
      textStyle: { color: "#8b949e" },
    },
    series: [
      {
        type: "heatmap",
        data: ratios.map((r, i) => [i, 0, Number(r.toFixed(2))]),
        label: { show: true, formatter: (p: unknown) => `${(p as { data: number[] }).data[2]}%` },
      },
    ],
  });
</script>

<div class="chart" use:chart={option}></div>
