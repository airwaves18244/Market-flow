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
  import SummaryPanel from "./lib/components/SummaryPanel.svelte";
  import BacktestPanel from "./lib/components/BacktestPanel.svelte";
  import TradePanel from "./lib/components/TradePanel.svelte";
  import { loadSettings, saveSettings, type Settings } from "./lib/settings";
  import { ipc, onTrade, onOrderBook } from "./lib/ipc";
  import type { AlertRuleInput, BarPoint, BondIssuerDto, BreadthDto, CrossAssetSummaryDto, FlowEdgeDto, FutureGroupDto, InstrumentDto, OrderBookDto, RegimeSignalDto, RrgSectorDto, SectorRow, TopMoverDto, TradeDto, TurnoverByClassPoint, YieldCurvePoint } from "./lib/types";

  // ── Рабочие пространства (левый рейл) ───────────────────────────────────
  type WsId =
    | "overview" | "summary" | "sectors" | "flows" | "live"
    | "futures" | "bonds" | "backtest" | "trade" | "settings";

  const WORKSPACES: Array<{ id: WsId; ru: string; en: string }> = [
    { id: "overview", ru: "Обзор", en: "Overview" },
    { id: "summary", ru: "Сводка", en: "Summary" },
    { id: "sectors", ru: "Секторы", en: "Sectors" },
    { id: "flows", ru: "Потоки", en: "Flows" },
    { id: "live", ru: "Лента", en: "Tape & DOM" },
    { id: "futures", ru: "Фьючерсы", en: "Futures" },
    { id: "bonds", ru: "Облигации", en: "Bonds" },
    { id: "backtest", ru: "Бэктест", en: "Backtest" },
    { id: "trade", ru: "Торговля", en: "Trade" },
    { id: "settings", ru: "Настройки", en: "Settings" },
  ];

  let ws = $state<WsId>("overview");

  // ── Период (глобальный): отображается как fromTs/toTs в IPC ──────────────
  const PERIODS: Array<{ k: string; ru: string; days: number }> = [
    { k: "d", ru: "1Д", days: 1 },
    { k: "w", ru: "1Н", days: 7 },
    { k: "m", ru: "1М", days: 30 },
    { k: "q", ru: "3М", days: 90 },
    { k: "ytd", ru: "YTD", days: 0 },
    { k: "y", ru: "1Г", days: 365 },
  ];
  let period = $state("m");

  function windowFor(key: string): { from: number; to: number } {
    const to = Math.floor(Date.now() / 1000);
    if (key === "ytd") {
      const from = Math.floor(Date.UTC(new Date().getUTCFullYear(), 0, 1) / 1000);
      return { from, to };
    }
    const days = PERIODS.find((p) => p.k === key)?.days ?? 30;
    return { from: to - days * 86_400, to };
  }

  // ── Данные ──────────────────────────────────────────────────────────────
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
  let regime = $state<RegimeSignalDto | null>(null);
  let trades = $state<TradeDto[]>([]);
  let orderBook = $state<OrderBookDto | null>(null);
  let selected = $state("SBER@MISX");
  let error = $state<string | null>(null);
  let settings = $state<Settings>(loadSettings());

  const selectedName = $derived(
    instruments.find((i) => i.symbol === selected)?.name ?? selected,
  );
  const lastClose = $derived(bars.length ? bars[bars.length - 1].close : 0);

  async function loadSymbol(symbol: string) {
    [bars, trades, orderBook] = await Promise.all([
      ipc.bars(symbol, "d1", 0, FULL_RANGE),
      ipc.latestTrades(symbol, settings.tapeLimit),
      ipc.orderBook(symbol, settings.domDepth),
    ]);
  }

  // Период-зависимая аналитика. Окно [from,to] идёт в каждую IPC-команду
  // (в браузерном мок-режиме аргументы игнорируются — данные те же).
  async function loadAggregates() {
    const { from, to } = windowFor(period);
    [sectors, breadth, topMovers, rrgData, futures, bonds, summary, timeline, flow, regime] =
      await Promise.all([
        ipc.sectorRollup(from, to),
        ipc.breadthData(from, to),
        ipc.topMovers(from, to, settings.topMoversLimit),
        ipc.rrgSectors(from, to),
        ipc.futuresRollup(from, to),
        ipc.bondsRollup(from, to),
        ipc.crossAssetSummary(from, to),
        ipc.turnoverTimeline(from, to),
        ipc.flowSankey(from, to),
        ipc.summary(from, to),
      ]);
  }

  async function setPeriod(k: string) {
    period = k;
    try {
      await loadAggregates();
    } catch (e) {
      error = String(e);
    }
  }

  async function select(symbol: string) {
    selected = symbol;
    try {
      await loadSymbol(symbol);
    } catch (e) {
      error = String(e);
    }
  }

  async function applySettings(next: Settings) {
    settings = next;
    saveSettings(next);
    try {
      const { from, to } = windowFor(period);
      topMovers = await ipc.topMovers(from, to, settings.topMoversLimit);
      await loadSymbol(selected);
    } catch (e) {
      error = String(e);
    }
  }

  const scanAlerts = (rules: AlertRuleInput[]) => ipc.alertsScan(rules, 0, FULL_RANGE);

  let unlisteners: Array<() => void> = [];

  onMount(async () => {
    try {
      instruments = await ipc.instruments();
      yieldCurve = await ipc.yieldCurve();
      await loadAggregates();
      if (instruments.length > 0) selected = instruments[0].symbol;
      await loadSymbol(selected);
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

  const activeWs = $derived(WORKSPACES.find((w) => w.id === ws) ?? WORKSPACES[0]);
</script>

<div class="shell">
  <!-- Верхняя панель -->
  <header class="topbar">
    <div class="brand">
      <span class="logo"></span>
      <div class="brand-text">
        <strong>MARKET TERMINAL</strong>
        <small>Оборот · потоки · Finam</small>
      </div>
    </div>
    <div class="period">
      <span class="period-cap">Период</span>
      {#each PERIODS as p (p.k)}
        <button class="pill" class:active={period === p.k} onclick={() => setPeriod(p.k)}>{p.ru}</button>
      {/each}
    </div>
    <div class="spacer"></div>
    <span class="live"><span class="live-dot"></span>LIVE</span>
    <span class="ro-tag">READ-ONLY v1</span>
  </header>

  <div class="body">
    <!-- Левый рейл -->
    <nav class="rail">
      {#each WORKSPACES as w (w.id)}
        <button class="rail-btn" class:active={ws === w.id} onclick={() => (ws = w.id)}>
          <span class="rail-ru">{w.ru}</span>
          <span class="rail-en">{w.en}</span>
        </button>
      {/each}
    </nav>

    <!-- Рабочая область -->
    <main class="work">
      <div class="work-head">
        <div class="title">
          <span class="t-ru">{activeWs.ru}</span>
          <span class="t-en">{activeWs.en}</span>
        </div>
        <span class="meta">{instruments.length} инстр · {sectors.length} секторов · диапазон {PERIODS.find((p) => p.k === period)?.ru}</span>
      </div>

      {#if error}
        <div class="error">Ошибка загрузки: {error}</div>
      {/if}

      {#if ws === "overview"}
        <div class="grid">
          <Panel title="Секторы — оборот (treemap)"><SectorTreemap rows={sectors} /></Panel>
          {#if summary}
            <Panel title="Общий оборот"><TotalTurnoverGauge total={summary.total} /></Panel>
            <Panel title="Доли классов"><SharesDonut shares={summary.shares} /></Panel>
          {/if}
          {#if breadth}
            <Panel title="Ширина рынка"><BreadthIndicator data={breadth} /></Panel>
          {/if}
          <Panel title="Топ-движения"><TopMoversTable movers={topMovers} /></Panel>
          <Panel title="Инструменты"><InstrumentList items={instruments} {selected} onSelect={select} /></Panel>
          <Panel title="Оборот во времени"><TurnoverStackedArea points={timeline} /></Panel>
        </div>
      {:else if ws === "summary"}
        <div class="single">
          {#if regime}
            <Panel title="Сводка — куда идут большие деньги"><SummaryPanel signal={regime} /></Panel>
          {/if}
        </div>
      {:else if ws === "sectors"}
        <div class="grid">
          <Panel title="Секторы — оборот (treemap)"><SectorTreemap rows={sectors} /></Panel>
          <Panel title="RRG — ротация секторов"><RrgChart sectors={rrgData} /></Panel>
          <Panel title="Секторы — тепловая карта (%)"><HeatmapChart {sectors} /></Panel>
        </div>
      {:else if ws === "flows"}
        <div class="grid">
          <Panel title="Перетоки (Sankey)"><FlowSankey edges={flow} /></Panel>
          <Panel title="Доли классов"><SharesDonut shares={summary?.shares ?? []} /></Panel>
          <Panel title="Оборот во времени"><TurnoverStackedArea points={timeline} /></Panel>
        </div>
      {:else if ws === "live"}
        <div class="grid live-grid">
          <Panel title={`Свечи — ${selected}`}><CandleChart {bars} /></Panel>
          <Panel title={`Стакан (DOM) — ${selected}`}><OrderBook book={orderBook} /></Panel>
          <Panel title={`Лента сделок — ${selected}`}><TimeSales {trades} /></Panel>
          <Panel title="Алёрты"><AlertsPanel {instruments} scan={scanAlerts} /></Panel>
        </div>
      {:else if ws === "futures"}
        <div class="grid">
          {#if futures.length > 0}
            <Panel title="Фьючерсы — группы (treemap)"><FuturesTreemap {futures} /></Panel>
          {/if}
        </div>
      {:else if ws === "bonds"}
        <div class="grid">
          <Panel title="Облигации — кривая доходности"><YieldCurve curve={yieldCurve} /></Panel>
          {#if bonds.length > 0}
            <Panel title="Облигации — эмитенты"><BondsTable issuers={bonds} /></Panel>
          {/if}
        </div>
      {:else if ws === "backtest"}
        <div class="single"><BacktestPanel /></div>
      {:else if ws === "trade"}
        <div class="single"><TradePanel symbol={selected.split("@")[0]} price={lastClose || undefined} /></div>
      {:else if ws === "settings"}
        <div class="grid">
          <Panel title="Настройки представления"><SettingsPanel {settings} onChange={applySettings} /></Panel>
        </div>
      {/if}
    </main>
  </div>

  <!-- Статус-бар -->
  <footer class="statusbar">
    <span><span class="dim">источник</span> Finam Trade API · gRPC</span>
    <span><span class="dim">DuckDB</span> schema</span>
    <span><span class="dim">инстр.</span> {instruments.length}</span>
    <span class="ro"><span class="ro-dot"></span>READ-ONLY v1 · только чтение</span>
  </footer>
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 46px;
    flex: none;
    padding: 0 14px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-panel);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .logo {
    width: 18px;
    height: 18px;
    border-radius: 3px;
    background: linear-gradient(135deg, var(--up), var(--accent) 60%, var(--down));
  }
  .brand-text {
    display: flex;
    flex-direction: column;
    line-height: 1.05;
  }
  .brand-text strong {
    font-size: 12.5px;
    letter-spacing: 0.05em;
  }
  .brand-text small {
    font-size: 8px;
    color: var(--text-dim);
    letter-spacing: 0.22em;
    text-transform: uppercase;
  }
  .period {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .period-cap {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-dim);
    margin-right: 4px;
  }
  .pill {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-dim);
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-variant-numeric: tabular-nums;
  }
  .pill.active {
    background: rgba(79, 156, 249, 0.18);
    color: var(--text);
    border-color: transparent;
  }
  .spacer {
    flex: 1;
  }
  .live {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-dim);
  }
  .live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--up);
  }
  .ro-tag {
    font-size: 10px;
    color: #e0a23a;
    border: 1px solid rgba(224, 162, 58, 0.4);
    border-radius: 4px;
    padding: 2px 7px;
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .rail {
    width: 84px;
    flex: none;
    border-right: 1px solid var(--border);
    background: var(--bg);
    display: flex;
    flex-direction: column;
    padding: 6px 0;
    overflow-y: auto;
  }
  .rail-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 9px 2px;
    border: none;
    border-left: 2px solid transparent;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
  }
  .rail-btn.active {
    border-left-color: var(--accent);
    background: rgba(79, 156, 249, 0.1);
    color: var(--text);
  }
  .rail-ru {
    font-size: 11px;
    font-weight: 600;
  }
  .rail-en {
    font-size: 8px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--text-dim);
  }
  .work {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .work-head {
    height: 38px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 14px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .t-ru {
    font-size: 14px;
    font-weight: 600;
  }
  .t-en {
    font-size: 9.5px;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--text-dim);
  }
  .meta {
    font-size: 10px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .grid {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    grid-auto-rows: minmax(260px, auto);
    gap: 8px;
    padding: 8px;
    min-height: 0;
    overflow: auto;
  }
  .grid > :global(:first-child) {
    grid-column: span 2;
    grid-row: span 2;
  }
  .live-grid > :global(:first-child) {
    grid-column: span 2;
  }
  .single {
    flex: 1;
    padding: 8px;
    overflow: auto;
    min-height: 0;
  }
  .error {
    margin: 8px;
    padding: 8px 12px;
    border: 1px solid var(--down);
    border-radius: 6px;
    color: var(--down);
    background: rgba(239, 83, 80, 0.08);
  }
  .statusbar {
    height: 25px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 14px;
    border-top: 1px solid var(--border);
    background: var(--bg);
    font-size: 10px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .statusbar .dim {
    color: var(--text-dim);
    opacity: 0.7;
  }
  .ro {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    color: #e0a23a;
  }
  .ro-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #e0a23a;
  }
</style>
