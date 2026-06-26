<script lang="ts">
  import { onMount } from "svelte";
  import Panel from "./lib/components/Panel.svelte";
  import SectorTreemap from "./lib/components/SectorTreemap.svelte";
  import CandleChart from "./lib/components/CandleChart.svelte";
  import InstrumentList from "./lib/components/InstrumentList.svelte";
  import BreadthIndicator from "./lib/components/BreadthIndicator.svelte";
  import TopMoversTable from "./lib/components/TopMoversTable.svelte";
  import HeatmapChart from "./lib/components/HeatmapChart.svelte";
  import RrgChart from "./lib/components/RrgChart.svelte";
  import FuturesTreemap from "./lib/components/FuturesTreemap.svelte";
  import YieldCurve from "./lib/components/YieldCurve.svelte";
  import BondsTable from "./lib/components/BondsTable.svelte";
  import { ipc } from "./lib/ipc";
  import type { BarPoint, BondIssuerDto, BreadthDto, FutureGroupDto, InstrumentDto, RrgSectorDto, SectorRow, TopMoverDto, YieldCurvePoint } from "./lib/types";

  const FULL_RANGE = Number.MAX_SAFE_INTEGER;

  let instruments = $state<InstrumentDto[]>([]);
  let sectors = $state<SectorRow[]>([]);
  let bars = $state<BarPoint[]>([]);
  let breadth = $state<BreadthDto | null>(null);
  let topMovers = $state<TopMoverDto[]>([]);
  let rrgData = $state<RrgSectorDto[]>([]);
  let futures = $state<FutureGroupDto[]>([]);
  let bonds = $state<BondIssuerDto[]>([]);
  let yieldCurve = $state<YieldCurvePoint[]>([]);
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
      breadth = await ipc.breadthData(0, FULL_RANGE);
      topMovers = await ipc.topMovers(0, FULL_RANGE, 10);
      rrgData = await ipc.rrgSectors(0, FULL_RANGE);
      futures = await ipc.futuresRollup(0, FULL_RANGE);
      bonds = await ipc.bondsRollup(0, FULL_RANGE);
      yieldCurve = await ipc.yieldCurve();
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

    {#if breadth}
      <Panel title="Market Breadth">
        <BreadthIndicator data={breadth} />
      </Panel>
    {/if}

    <Panel title="Top Movers">
      <TopMoversTable movers={topMovers} />
    </Panel>

    <Panel title="Heatmap">
      <HeatmapChart {sectors} />
    </Panel>

    {#if rrgData.length > 0}
      <Panel title="RRG — Sector Rotation">
        <RrgChart sectors={rrgData} />
      </Panel>
    {/if}

    {#if futures.length > 0}
      <Panel title="Futures — Groups (Treemap)">
        <FuturesTreemap {futures} />
      </Panel>
    {/if}

    {#if yieldCurve.length > 0}
      <Panel title="Bonds — Yield Curve">
        <YieldCurve curve={yieldCurve} />
      </Panel>
    {/if}

    {#if bonds.length > 0}
      <Panel title="Bonds — Issuers">
        <BondsTable issuers={bonds} />
      </Panel>
    {/if}
  </main>
</div>
