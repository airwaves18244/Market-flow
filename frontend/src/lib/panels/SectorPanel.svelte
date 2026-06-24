<script lang="ts">
  import type { EChartsOption } from "echarts";
  import { chart } from "../charts";
  import { store } from "../store.svelte";

  // Treemap: размер прямоугольника — оборот, цвет — знак нетто-потока.
  const option = $derived<EChartsOption>({
    tooltip: {
      formatter: (p: unknown) => {
        const item = p as { name: string; value: number };
        return `${item.name}<br/>оборот: ${item.value.toLocaleString("ru-RU")}`;
      },
    },
    series: [
      {
        type: "treemap",
        roam: false,
        breadcrumb: { show: false },
        label: { show: true, formatter: "{b}" },
        data: (store.equity?.sectors ?? []).map((s) => ({
          name: s.sector,
          value: s.turnover,
          itemStyle: { color: s.net_flow >= 0 ? "#2e7d32" : "#c62828" },
        })),
      },
    ],
  });
</script>

<div class="chart" use:chart={option}></div>
