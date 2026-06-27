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
  import TotalTurnoverGauge from "./lib/components/TotalTurnoverGauge.svelte";
  import SharesDonut from "./lib/components/SharesDonut.svelte";
  import TurnoverStackedArea from "./lib/components/TurnoverStackedArea.svelte";
  import FlowSankey from "./lib/components/FlowSankey.svelte";
  import DomLadder from "./lib/components/DomLadder.svelte";
  import TimeSales from "./lib/components/TimeSales.svelte";
  import AlertsPanel from "./lib/components/AlertsPanel.svelte";
  import ReplayControls from "./lib/components/ReplayControls.svelte";
  import { ipc } from "./lib/ipc";
  import type { BarPoint, BondIssuerDto, BreadthDto, CrossAssetSummaryDto, FlowEdgeDto, FutureGroupDto, InstrumentDto, OrderBookDto, ReplayStateDto, RrgSectorDto, SectorRow, TimeAndSalesDto, TopMoverDto, TriggeredAlertDto, TurnoverByClassPoint, YieldCurvePoint } from "./lib/types";

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
  let summary = $state<CrossAssetSummaryDto | null>(null);
  let timeline = $state<TurnoverByClassPoint[]>([]);
  let flow = $state<FlowEdgeDto[]>([]);
  let orderBook = $state<OrderBookDto | null>(null);
  let tape = $state<TimeAndSalesDto | null>(null);
  let alerts = $state<TriggeredAlertDto[]>([]);
  let replay = $state<ReplayStateDto | null>(null);
  let selected = $state("SBER@MISX");
  let error = $state<string | null>(null);

  async function loadBars(symbol: string) {
    bars = await ipc.bars(symbol, "d1", 0, FULL_RANGE);
  }

  // Live-данные привязаны к выбранному инструменту (Фаза 7).
  async function loadLive(symbol: string) {
    orderBook = await ipc.orderBook(symbol, 12);
    tape = await ipc.timeAndSales(symbol, 40);
    replay = await ipc.replayState(symbol, 0);
  }

  async function seekReplay(played: number) {
    try {
      replay = await ipc.replayState(selected, played);
    } catch (e) {
      error = String(e);
    }
  }

  async function select(symbol: string) {
    selected = symbol;
    try {
      await loadBars(symbol);
      await loadLive(symbol);
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
      summary = await ipc.crossAssetSummary(0, FULL_RANGE);
      timeline = await ipc.turnoverTimeline(0, FULL_RANGE);
      flow = await ipc.flowSankey(0, FULL_RANGE);
      alerts = await ipc.activeAlerts();
      if (instruments.length > 0) selected = instruments[0].symbol;
      await loadBars(selected);
      await loadLive(selected);
    } catch (e) {
      error = String(e);
    }
  });
</script>

<div class="app">
  <header class="app-header">
    <h1>Market Terminal</h1>
    <span class="sub">Акции · фьючерсы · облигации — оборот и денежные потоки</span>
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
      <Panel title="Ширина рынка">
        <BreadthIndicator data={breadth} />
      </Panel>
    {/if}

    <Panel title="Топ-движения">
      <TopMoversTable movers={topMovers} />
    </Panel>

    <Panel title="Секторы — тепловая карта (%)">
      <HeatmapChart {sectors} />
    </Panel>

    <Panel title="RRG — ротация секторов">
      <RrgChart sectors={rrgData} />
    </Panel>

    {#if futures.length > 0}
      <Panel title="Фьючерсы — группы (treemap)">
        <FuturesTreemap {futures} />
      </Panel>
    {/if}

    <Panel title="Облигации — кривая доходности">
      <YieldCurve curve={yieldCurve} />
    </Panel>

    {#if bonds.length > 0}
      <Panel title="Облигации — эмитенты">
        <BondsTable issuers={bonds} />
      </Panel>
    {/if}

    {#if summary}
      <Panel title="Сумма всех — общий оборот">
        <TotalTurnoverGauge total={summary.total} />
      </Panel>

      <Panel title="Сумма всех — доли классов">
        <SharesDonut shares={summary.shares} />
      </Panel>
    {/if}

    <Panel title="Сумма всех — оборот во времени">
      <TurnoverStackedArea points={timeline} />
    </Panel>

    <Panel title="Сумма всех — перетоки (Sankey)">
      <FlowSankey edges={flow} />
    </Panel>

    <Panel title={`Стакан (DOM) — ${selected}`}>
      <DomLadder book={orderBook} />
    </Panel>

    <Panel title={`Лента сделок — ${selected}`}>
      <TimeSales {tape} />
    </Panel>

    <Panel title="Алёрты">
      <AlertsPanel {alerts} />
    </Panel>

    <Panel title={`Replay — ${selected}`}>
      <ReplayControls replay={replay} onSeek={seekReplay} />
    </Panel>
  </main>
</div>
