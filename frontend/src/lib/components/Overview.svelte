<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Panel from "./Panel.svelte";
  import SectorTreemap from "./SectorTreemap.svelte";
  import CandleChart from "./CandleChart.svelte";
  import InstrumentList from "./InstrumentList.svelte";
  import BreadthIndicator from "./BreadthIndicator.svelte";
  import TopMoversTable from "./TopMoversTable.svelte";
  import HeatmapChart from "./HeatmapChart.svelte";
  import RrgChart from "./RrgChart.svelte";
  import FuturesTreemap from "./FuturesTreemap.svelte";
  import YieldCurve from "./YieldCurve.svelte";
  import BondsTable from "./BondsTable.svelte";
  import TotalTurnoverGauge from "./TotalTurnoverGauge.svelte";
  import SharesDonut from "./SharesDonut.svelte";
  import TurnoverStackedArea from "./TurnoverStackedArea.svelte";
  import FlowSankey from "./FlowSankey.svelte";
  import TimeSales from "./TimeSales.svelte";
  import OrderBook from "./OrderBook.svelte";
  import AlertsPanel from "./AlertsPanel.svelte";
  import SettingsPanel from "./SettingsPanel.svelte";
  import { loadSettings, saveSettings, type Settings } from "../settings";
  import { ipc, onTrade, onOrderBook } from "../ipc";
  import type { AlertRuleInput, BarPoint, BondIssuerDto, BreadthDto, CrossAssetSummaryDto, FlowEdgeDto, FutureGroupDto, InstrumentDto, OrderBookDto, RrgSectorDto, SectorRow, TopMoverDto, TradeDto, TurnoverByClassPoint, YieldCurvePoint } from "../types";

  let {
    instruments,
    selected,
    onSelect,
  }: {
    instruments: InstrumentDto[];
    selected: string;
    onSelect: (symbol: string) => void;
  } = $props();

  const FULL_RANGE = Number.MAX_SAFE_INTEGER;

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
  let trades = $state<TradeDto[]>([]);
  let orderBook = $state<OrderBookDto | null>(null);
  let error = $state<string | null>(null);
  let settings = $state<Settings>(loadSettings());

  async function loadSymbol() {
    [bars, trades, orderBook] = await Promise.all([
      ipc.bars(selected, "d1", 0, FULL_RANGE),
      ipc.latestTrades(selected, settings.tapeLimit),
      ipc.orderBook(selected, settings.domDepth),
    ]);
  }

  // Перезагрузка зависящих от выбранного инструмента/настроек данных.
  $effect(() => {
    void selected;
    void settings;
    loadSymbol().catch((e) => (error = String(e)));
  });

  async function applySettings(next: Settings) {
    settings = next;
    saveSettings(next);
    try {
      topMovers = await ipc.topMovers(0, FULL_RANGE, settings.topMoversLimit);
    } catch (e) {
      error = String(e);
    }
  }

  const scanAlerts = (rules: AlertRuleInput[]) => ipc.alertsScan(rules, 0, FULL_RANGE);

  let unlisteners: Array<() => void> = [];
  onMount(async () => {
    try {
      sectors = await ipc.sectorRollup(0, FULL_RANGE);
      breadth = await ipc.breadthData(0, FULL_RANGE);
      topMovers = await ipc.topMovers(0, FULL_RANGE, settings.topMoversLimit);
      rrgData = await ipc.rrgSectors(0, FULL_RANGE);
      futures = await ipc.futuresRollup(0, FULL_RANGE);
      bonds = await ipc.bondsRollup(0, FULL_RANGE);
      yieldCurve = await ipc.yieldCurve();
      summary = await ipc.crossAssetSummary(0, FULL_RANGE);
      timeline = await ipc.turnoverTimeline(0, FULL_RANGE);
      flow = await ipc.flowSankey(0, FULL_RANGE);
    } catch (e) {
      error = String(e);
    }

    unlisteners.push(
      await onTrade((t) => {
        trades = [t, ...trades].slice(0, settings.tapeLimit);
      }),
      await onOrderBook((b) => {
        orderBook = b;
      }),
    );
  });

  onDestroy(() => {
    for (const un of unlisteners) un();
  });
</script>

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
    <InstrumentList items={instruments} {selected} onSelect={onSelect} />
  </Panel>

  <Panel title={`Лента сделок — ${selected}`}>
    <TimeSales {trades} />
  </Panel>

  <Panel title={`Стакан (DOM) — ${selected}`}>
    <OrderBook book={orderBook} />
  </Panel>

  <Panel title="Алёрты">
    <AlertsPanel {instruments} scan={scanAlerts} />
  </Panel>

  <Panel title="Настройки">
    <SettingsPanel {settings} onChange={applySettings} />
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
</main>
