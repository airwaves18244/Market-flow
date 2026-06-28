<script lang="ts">
  import { onMount, onDestroy } from "svelte";
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
  import TimeSales from "./lib/components/TimeSales.svelte";
  import OrderBook from "./lib/components/OrderBook.svelte";
  import AlertsPanel from "./lib/components/AlertsPanel.svelte";
  import SettingsPanel from "./lib/components/SettingsPanel.svelte";
  import { loadSettings, saveSettings, type Settings } from "./lib/settings";
  import { ipc, onTrade, onOrderBook } from "./lib/ipc";
  import type { AlertRuleInput, BarPoint, BondIssuerDto, BreadthDto, CrossAssetSummaryDto, FlowEdgeDto, FutureGroupDto, InstrumentDto, OrderBookDto, RrgSectorDto, SectorRow, TopMoverDto, TradeDto, TurnoverByClassPoint, YieldCurvePoint } from "./lib/types";

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
  let trades = $state<TradeDto[]>([]);
  let orderBook = $state<OrderBookDto | null>(null);
  let selected = $state("SBER@MISX");
  let error = $state<string | null>(null);
  let settings = $state<Settings>(loadSettings());

  async function loadSymbol(symbol: string) {
    // Свечи + live-панели (Time&Sales / DOM) зависят от выбранного инструмента.
    [bars, trades, orderBook] = await Promise.all([
      ipc.bars(symbol, "d1", 0, FULL_RANGE),
      ipc.latestTrades(symbol, settings.tapeLimit),
      ipc.orderBook(symbol, settings.domDepth),
    ]);
  }

  async function select(symbol: string) {
    selected = symbol;
    try {
      await loadSymbol(symbol);
    } catch (e) {
      error = String(e);
    }
  }

  // Применить новые настройки: сохранить и перезагрузить зависимые данные.
  async function applySettings(next: Settings) {
    settings = next;
    saveSettings(next);
    try {
      topMovers = await ipc.topMovers(0, FULL_RANGE, settings.topMoversLimit);
      await loadSymbol(selected);
    } catch (e) {
      error = String(e);
    }
  }

  // Прогон правил алёртов через ядро (для панели алёртов).
  const scanAlerts = (rules: AlertRuleInput[]) => ipc.alertsScan(rules, 0, FULL_RANGE);

  onMount(async () => {
    try {
      instruments = await ipc.instruments();
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
      if (instruments.length > 0) selected = instruments[0].symbol;
      await loadSymbol(selected);
    } catch (e) {
      error = String(e);
    }

    // Live-push: в десктопной сборке лента/стакан обновляются событиями
    // (в браузере подписки — no-op, данные берутся из первичного снимка).
    unlisteners.push(
      await onTrade((t) => {
        trades = [t, ...trades].slice(0, settings.tapeLimit);
      }),
      await onOrderBook((b) => {
        orderBook = b;
      }),
    );
  });

  let unlisteners: Array<() => void> = [];
  onDestroy(() => {
    for (const un of unlisteners) un();
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
</div>
