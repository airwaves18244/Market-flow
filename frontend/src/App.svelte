<script lang="ts">
  import { onMount } from "svelte";
  import Panel from "./lib/components/Panel.svelte";
  import SectorTreemap from "./lib/components/SectorTreemap.svelte";
  import CandleChart from "./lib/components/CandleChart.svelte";
  import InstrumentList from "./lib/components/InstrumentList.svelte";
  import { ipc } from "./lib/ipc";
  import type { BarPoint, InstrumentDto, SectorRow } from "./lib/types";

  const FULL_RANGE = Number.MAX_SAFE_INTEGER;

  let instruments = $state<InstrumentDto[]>([]);
  let sectors = $state<SectorRow[]>([]);
  let bars = $state<BarPoint[]>([]);
  let selected = $state("SBER@MISX");
  let error = $state<string | null>(null);

  async function loadBars(symbol: string) {
    bars = await ipc.bars(symbol, "d1", 0, FULL_RANGE);
  }

  async function select(symbol: string) {
    selected = symbol;
    try {
      await loadBars(symbol);
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
    try {
      instruments = await ipc.instruments();
      sectors = await ipc.sectorRollup(0, FULL_RANGE);
      if (instruments.length > 0) selected = instruments[0].symbol;
      await loadBars(selected);
    } catch (e) {
      error = String(e);
    }
  });
</script>

<div class="app">
  <header class="app-header">
    <h1>Market Terminal</h1>
    <span class="sub">Акции / секторы — оборот и денежные потоки</span>
    <span class="status">{instruments.length} инструментов · {sectors.length} секторов</span>
  </header>

  {#if error}
    <div class="error">Ошибка загрузки: {error}</div>
  {/if}

  <main class="grid">
    <Panel title="Секторы — оборот (treemap)">
      <SectorTreemap rows={sectors} />
    </Panel>

    <Panel title={`Свечи — ${selected}`}>
      <CandleChart {bars} />
    </Panel>

    <Panel title="Инструменты">
      <InstrumentList items={instruments} {selected} onSelect={select} />
    </Panel>
  </main>
</div>
