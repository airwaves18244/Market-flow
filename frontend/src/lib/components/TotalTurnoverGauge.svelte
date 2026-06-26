<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as echarts from "echarts";

  let { total = 0 }: { total: number } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | undefined;
  let ro: ResizeObserver | undefined;

  // Шкала в млрд ₽; верх округляем вверх до «красивого» предела.
  function niceMax(v: number): number {
    if (v <= 0) return 1;
    const pow = Math.pow(10, Math.floor(Math.log10(v)));
    return Math.ceil(v / pow) * pow;
  }

  function render() {
    if (!chart) return;
    const bln = total / 1e9;
    const max = niceMax(bln);
    chart.setOption({
      backgroundColor: "transparent",
      series: [
        {
          type: "gauge",
          min: 0,
          max,
          progress: { show: true, width: 12, itemStyle: { color: "#4f9cf9" } },
          axisLine: { lineStyle: { width: 12, color: [[1, "#21262d"]] } },
          axisTick: { show: false },
          splitLine: { length: 10, lineStyle: { color: "#484f58" } },
          axisLabel: { color: "#8b949e", fontSize: 9, distance: 14 },
          pointer: { itemStyle: { color: "#4f9cf9" } },
          anchor: { show: true, size: 12, itemStyle: { color: "#4f9cf9" } },
          detail: {
            valueAnimation: true,
            formatter: (v: number) => `${v.toFixed(1)} млрд ₽`,
            color: "#e6edf3",
            fontSize: 16,
            offsetCenter: [0, "70%"],
          },
          data: [{ value: Number(bln.toFixed(2)) }],
        },
      ],
    });
  }

  $effect(() => {
    void total;
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

<div class="gauge" bind:this={el}></div>

<style>
  .gauge {
    width: 100%;
    height: 100%;
    min-height: 200px;
  }
</style>
