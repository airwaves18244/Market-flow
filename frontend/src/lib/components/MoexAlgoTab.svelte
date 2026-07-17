<script lang="ts">
  import { onMount } from "svelte";
  import KeyActivityTable from "./KeyActivityTable.svelte";
  import KeyActivitySummary from "./KeyActivitySummary.svelte";
  import SuperCandlesChart from "./SuperCandlesChart.svelte";
  import DisbBars from "./DisbBars.svelte";
  import FutoiChart from "./FutoiChart.svelte";
  import Hi2Chart from "./Hi2Chart.svelte";
  import { ipc, inTauri } from "../ipc";
  import { loadSettings } from "../settings";
  import { algoTickers } from "../mock";
  import { fmtRuFixed, fmtInt } from "../format";
  import type {
    AlgoMarket,
    FutoiDto,
    Hi2Dto,
    KeyActivityPeriod,
    KeyActivityRowDto,
    KeyActivitySampleInput,
    KeyActivitySummaryDto,
    MegaAlertDto,
    MegaAlertKind,
    TradestatsDto,
  } from "../types";

  // Вкладка «MOEX ALGO». Герой вкладки — «Ключевая активность» + «ИТОГО»
  // (задачи 10.7.6/10.7.7) на боевом IPC (`key_activity`). Остальные модули
  // (Супер-свечи, FUTOI, HI2, Мега-алёрты, T11) читают датасеты ALGOPACK через
  // типизированный `ipc.ts` (`algo_tradestats`/`algo_futoi`/`algo_hi2`/
  // `algo_mega_alerts`); в браузере без Tauri — те же команды отдают
  // детерминированный мок (`lib/mock.ts`).

  const settings = loadSettings();

  const modules = [
    { id: "key", label: "Ключевая активность + ИТОГО" },
    { id: "super", label: "Супер-свечи" },
    { id: "futoi", label: "FUTOI" },
    { id: "hi2", label: "Концентрация HI2" },
    { id: "mega", label: "Мега-алёрты" },
  ] as const;
  let active = $state<(typeof modules)[number]["id"]>("key");

  const periods: KeyActivityPeriod[] = ["1h", "1d", "1w", "1m", "3m"];
  const periodLabels: Record<KeyActivityPeriod, string> = {
    "1h": "1ч",
    "1d": "1д",
    "1w": "1н",
    "1m": "1м",
    "3m": "3м",
  };
  let period = $state<KeyActivityPeriod>(settings.defaultPeriod);

  const markets: { id: AlgoMarket; label: string }[] = [
    { id: "eq", label: "Акции" },
    { id: "fo", label: "Фьючерсы" },
    { id: "fx", label: "Валюта" },
  ];
  let market = $state<AlgoMarket>("eq");

  const universe = algoTickers();
  let ticker = $state("SBER");
  let futoiMode = $state<"long" | "short" | "net">("net");
  let megaType = $state<"all" | MegaAlertKind>("all");

  // Широкое окно запроса: датасеты ALGOPACK хранятся с реальными unix-метками
  // (боевой ингест) либо со синтетическими (мок) — обе стороны укладываются в
  // этот диапазон без знания точных границ на фронте.
  const FROM_TS = 0;
  const TO_TS = 9_999_999_999;

  function timeLabel(ts: number): string {
    const d = new Date(ts * 1000);
    return `${String(d.getUTCHours()).padStart(2, "0")}:${String(d.getUTCMinutes()).padStart(2, "0")}`;
  }

  // ── Key Activity (боевой IPC) ──────────────────────────────────────────────
  const demoSamples: KeyActivitySampleInput[] = [
    { secid: "SBER", ts: 4, volume: 5200, volumeZ: 3.8, disb: 0.55, hi2: 0.22, priceChange: 0.031 },
    { secid: "GAZP", ts: 4, volume: 900, volumeZ: 0.6, disb: -0.62, hi2: 0.71, priceChange: -0.008 },
    { secid: "LKOH", ts: 4, volume: 2100, volumeZ: 1.2, disb: 0.15, hi2: 0.34, priceChange: 0.026 },
    { secid: "GMKN", ts: 4, volume: 1500, volumeZ: 2.9, disb: 0.05, hi2: 0.28, priceChange: -0.004 },
  ];
  let rows = $state<KeyActivityRowDto[]>([]);
  let summary = $state<KeyActivitySummaryDto | null>(null);
  let loadingSummary = $state(false);

  async function loadRows() {
    rows = await ipc.keyActivity(demoSamples, period);
  }
  async function loadSummary() {
    loadingSummary = true;
    try {
      summary = await ipc.keyActivitySummary(demoSamples, period);
    } finally {
      loadingSummary = false;
    }
  }
  async function setPeriod(p: KeyActivityPeriod) {
    period = p;
    await loadRows();
    if (settings.llmAuto) await loadSummary();
  }

  // ── Супер-свечи / FUTOI / HI2 (боевой IPC: датасеты ALGOPACK, T11) ─────────
  let candles = $state<TradestatsDto[]>([]);
  let futoiPoints = $state<FutoiDto[]>([]);
  let hi2Points = $state<Hi2Dto[]>([]);
  let hi2Rank = $state<Hi2Dto[]>([]);
  let megaAlerts = $state<MegaAlertDto[]>([]);

  // Счётчик поколений запросов: при быстром переключении инструмента/рынка
  // ответы более старых запросов не должны затирать состояние свежих. Каждый
  // загрузчик фиксирует своё поколение и присваивает результат только если оно
  // всё ещё актуально. `loadAlgoData` задаёт одно поколение на весь пакет.
  let reqSeq = 0;

  async function loadTickerData(seq = ++reqSeq) {
    const [ts, fu, hi] = await Promise.all([
      ipc.algoTradestats(market, ticker, FROM_TS, TO_TS),
      ipc.algoFutoi(market, ticker, FROM_TS, TO_TS),
      ipc.algoHi2(market, ticker, FROM_TS, TO_TS),
    ]);
    if (seq !== reqSeq) return;
    candles = ts;
    futoiPoints = fu;
    hi2Points = hi;
  }

  async function loadHi2Ranking(seq = ++reqSeq) {
    // Батч «последние точки» одним вызовом вместо полной истории ×N тикеров
    // (сортировка и топ-10 — на стороне ядра).
    const ranking = await ipc.algoHi2Ranking(
      market,
      universe.map((t) => t.ticker),
      10,
    );
    if (seq !== reqSeq) return;
    hi2Rank = ranking;
  }

  async function loadMegaAlerts(seq = ++reqSeq) {
    const alerts = await ipc.algoMegaAlerts(
      market,
      universe.map((t) => t.ticker),
      FROM_TS,
      TO_TS,
    );
    if (seq !== reqSeq) return;
    megaAlerts = alerts;
  }

  async function loadAlgoData() {
    const seq = ++reqSeq;
    await Promise.all([loadTickerData(seq), loadHi2Ranking(seq), loadMegaAlerts(seq)]);
  }

  async function setTicker(t: string) {
    ticker = t;
    await loadTickerData();
  }

  async function setMarket(m: AlgoMarket) {
    market = m;
    await loadAlgoData();
  }

  onMount(async () => {
    await loadRows();
    await loadSummary();
    await loadAlgoData();
  });

  // ── Производные представления для остальных модулей ────────────────────────
  const scRows = $derived([...candles].slice(-14).reverse());

  interface FutoiRow {
    time: string;
    group: "Физлица" | "Юрлица";
    long: number;
    short: number;
    net: number;
    sharePct: number;
    doi: number;
  }
  const futoiRows = $derived.by((): FutoiRow[] => {
    const rows: FutoiRow[] = [];
    for (const [group, label] of [
      ["fiz", "Физлица"],
      ["yur", "Юрлица"],
    ] as const) {
      const pts = futoiPoints.filter((p) => p.clgroup === group).sort((a, b) => a.ts - b.ts);
      let prevNet: number | null = null;
      for (const p of pts) {
        rows.push({
          time: timeLabel(p.ts),
          group: label,
          long: p.posLong,
          short: p.posShort,
          net: p.net,
          sharePct: p.longShare * 100,
          doi: prevNet === null ? 0 : p.net - prevNet,
        });
        prevNet = p.net;
      }
    }
    return rows.reverse();
  });

  const megaTypes = $derived(["all", ...new Set(megaAlerts.map((a) => a.kind))] as const);
  const megaView = $derived(megaAlerts.filter((a) => megaType === "all" || a.kind === megaType));

  const MEGA_TYPE_LABELS: Record<MegaAlertKind, string> = {
    volume_spike: "Всплеск объёма",
    buy_imbalance: "Дисбаланс покупок",
    sell_imbalance: "Дисбаланс продаж",
    spread_widening: "Расширение спреда",
    oi_jump: "Скачок OI",
    concentration_rise: "Концентрация HI2",
  };
  const MEGA_SEVERITY: Record<MegaAlertKind, "high" | "med" | "low"> = {
    volume_spike: "high",
    buy_imbalance: "high",
    sell_imbalance: "high",
    spread_widening: "med",
    oi_jump: "med",
    concentration_rise: "low",
  };
  function megaTypeLabel(kind: string): string {
    return kind === "all" ? "Все типы" : (MEGA_TYPE_LABELS[kind as MegaAlertKind] ?? kind);
  }
  function megaSeverity(a: MegaAlertDto): "high" | "med" | "low" {
    return MEGA_SEVERITY[a.kind];
  }
  function megaUp(a: MegaAlertDto): boolean {
    return a.value >= 0;
  }
  function megaValueLabel(a: MegaAlertDto): string {
    switch (a.kind) {
      case "volume_spike":
        return "×" + fmtRuFixed(a.value);
      case "buy_imbalance":
        return "+" + fmtRuFixed(a.value, 2);
      case "sell_imbalance":
        return "−" + fmtRuFixed(Math.abs(a.value), 2);
      case "spread_widening":
        return "+" + fmtRuFixed(a.value * 10_000) + " бп";
      case "oi_jump":
        return (a.value >= 0 ? "+" : "−") + fmtInt(Math.abs(a.value) / 1000) + "k";
      default:
        return fmtRuFixed(a.value, 2);
    }
  }

  const sevLabel = (s: string) => (s === "high" ? "высокая" : s === "med" ? "средняя" : "низкая");
  const HI2_LEVEL_LABELS: Record<Hi2Dto["level"], string> = {
    distributed: "распределённая",
    moderate: "умеренная",
    concentrated: "концентрация",
    dominated: "доминирование",
  };
  const hi2LevelLabel = (level: Hi2Dto["level"]) => HI2_LEVEL_LABELS[level];
  const hi2Color = (level: Hi2Dto["level"]) =>
    level === "dominated" || level === "concentrated"
      ? "var(--down)"
      : level === "moderate"
        ? "#f5a623"
        : "var(--up)";
</script>

<div class="algo">
  <!-- Тулбар: инструмент · период · рынок -->
  <div class="toolbar">
    <span class="tb-label">Инструмент</span>
    <select class="ctl-sm" bind:value={ticker} onchange={() => setTicker(ticker)}>
      {#each universe as t (t.ticker)}
        <option value={t.ticker}>{t.ticker} · {t.name}</option>
      {/each}
    </select>
    <span class="tb-label">Период</span>
    <div class="seg-wrap">
      {#each periods as p (p)}
        <button class="seg-btn" class:active={p === period} onclick={() => setPeriod(p)}>
          {periodLabels[p]}
        </button>
      {/each}
    </div>
    <span class="tb-label">Рынок</span>
    <div class="seg-wrap">
      {#each markets as mk (mk.id)}
        <button class="seg-btn" class:active={mk.id === market} onclick={() => setMarket(mk.id)}>
          {mk.label}
        </button>
      {/each}
    </div>
    <span class="status-line"
      >{universe.length} инструментов · {ticker} · {inTauri() ? "боевой режим" : "мок-режим"}</span
    >
  </div>

  <!-- Переключатель модулей -->
  <nav class="modules">
    {#each modules as m (m.id)}
      <button class="mod-btn" class:active={m.id === active} onclick={() => (active = m.id)}>
        {m.label}
      </button>
    {/each}
  </nav>

  {#if active === "key"}
    <div class="hero">
      <KeyActivityTable {rows} {period} onPeriod={setPeriod} />
      <KeyActivitySummary {summary} loading={loadingSummary} onRefresh={loadSummary} />
    </div>
  {:else if active === "super"}
    <div class="stack">
      <section class="panel">
        <header class="panel-head">
          <span>Супер-свечи · {ticker}</span>
          <span class="head-sub">5-мин · VWAP-полоса · дисбаланс</span>
        </header>
        <div class="panel-body">
          <div class="chart-lg"><SuperCandlesChart bars={candles} /></div>
          <div class="sub-label">Дисбаланс покупок/продаж</div>
          <DisbBars bars={candles} />
        </div>
      </section>
      <section class="panel">
        <header class="panel-head">Метрики (последние бары)</header>
        <div class="panel-body pad0">
          <table>
            <thead>
              <tr>
                <th>Время</th><th class="num">O</th><th class="num">H</th><th class="num">L</th>
                <th class="num">C</th><th class="num">VWAP</th><th class="num">Объём</th>
                <th class="num">Сделки</th><th class="num">disb</th>
              </tr>
            </thead>
            <tbody>
              {#each scRows as b (b.ts)}
                <tr class:anom={b.isAnomVol}>
                  <td class="dim">{timeLabel(b.ts)}</td>
                  <td class="num">{fmtRuFixed(b.prOpen)}</td><td class="num">{fmtRuFixed(b.prHigh)}</td>
                  <td class="num">{fmtRuFixed(b.prLow)}</td><td class="num">{fmtRuFixed(b.prClose)}</td>
                  <td class="num vwap">{fmtRuFixed(b.prVwap)}</td><td class="num">{fmtInt(b.vol)}</td>
                  <td class="num">{fmtInt(b.trades)}</td>
                  <td class="num" class:up={b.disb >= 0} class:down={b.disb < 0}>
                    {(b.disb >= 0 ? "+" : "") + fmtRuFixed(b.disb, 2)}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  {:else if active === "futoi"}
    <div class="stack">
      <section class="panel">
        <header class="panel-head">
          <span>Открытые позиции (FUTOI) · физ/юр</span>
          <div class="seg-wrap head-right">
            {#each [{ id: "long", label: "Long" }, { id: "short", label: "Short" }, { id: "net", label: "Нетто" }] as f (f.id)}
              <button
                class="seg-btn"
                class:active={f.id === futoiMode}
                onclick={() => (futoiMode = f.id as typeof futoiMode)}>{f.label}</button
              >
            {/each}
          </div>
        </header>
        <div class="panel-body"><FutoiChart points={futoiPoints} mode={futoiMode} /></div>
      </section>
      <section class="panel">
        <header class="panel-head">Позиции по группам</header>
        <div class="panel-body pad0">
          <table>
            <thead>
              <tr>
                <th>Время</th><th>Группа</th><th class="num">Long</th><th class="num">Short</th>
                <th class="num">Нетто</th><th class="num">Доля long</th><th class="num">ΔOI</th>
              </tr>
            </thead>
            <tbody>
              {#each futoiRows as r, i (i)}
                <tr>
                  <td class="dim">{r.time}</td>
                  <td class="grp" style:color={r.group === "Физлица" ? "var(--accent)" : "#f5a623"}>
                    {r.group}
                  </td>
                  <td class="num">{fmtInt(r.long)}</td>
                  <td class="num">{fmtInt(r.short)}</td>
                  <td class="num" class:up={r.net >= 0} class:down={r.net < 0}>
                    {(r.net >= 0 ? "+" : "−") + fmtInt(Math.abs(r.net))}
                  </td>
                  <td class="num">{fmtRuFixed(r.sharePct, 1)}%</td>
                  <td class="num" class:up={r.doi >= 0} class:down={r.doi < 0}>
                    {(r.doi >= 0 ? "+" : "−") + fmtInt(Math.abs(r.doi))}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  {:else if active === "hi2"}
    <div class="stack">
      <section class="panel">
        <header class="panel-head">Концентрация рынка (HI2) · {ticker}</header>
        <div class="panel-body"><Hi2Chart points={hi2Points} /></div>
      </section>
      <section class="panel">
        <header class="panel-head">Ранжирование по концентрации</header>
        <div class="panel-body pad0">
          <table>
            <thead>
              <tr><th>Тикер</th><th class="num">HI2</th><th>Уровень</th><th class="ctr">Всплеск</th></tr>
            </thead>
            <tbody>
              {#each hi2Rank as r (r.secid)}
                <tr>
                  <td class="tk">{r.secid}</td>
                  <td class="num">{fmtRuFixed(r.concentration, 3)}</td>
                  <td style:color={hi2Color(r.level)}>{hi2LevelLabel(r.level)}</td>
                  <td class="ctr">{r.spike ? "⚠" : ""}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  {:else if active === "mega"}
    <section class="panel">
      <header class="panel-head">
        <span>Мега-алёрты</span>
        <select class="ctl-sm head-right" bind:value={megaType}>
          {#each megaTypes as t (t)}
            <option value={t}>{megaTypeLabel(t)}</option>
          {/each}
        </select>
      </header>
      <div class="panel-body">
        <div class="alerts">
          {#each megaView as a, i (i)}
            {@const sev = megaSeverity(a)}
            <div
              class="alert"
              style:border-left-color={sev === "high"
                ? "var(--down)"
                : sev === "med"
                  ? "#f5a623"
                  : "var(--text-dim)"}
            >
              <span class="a-time">{timeLabel(a.ts)}</span>
              <span class="a-tk">{a.secid}</span>
              <span class="a-type">{megaTypeLabel(a.kind)}</span>
              <span class="a-metric">{a.message}</span>
              <span class="a-val" class:up={megaUp(a)} class:down={!megaUp(a)}
                >{megaValueLabel(a)}</span
              >
              <span class="sev {sev}">{sevLabel(sev)}</span>
            </div>
          {/each}
        </div>
      </div>
    </section>
  {/if}
</div>

<style>
  .algo {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .status-line {
    margin-left: auto;
    color: var(--text-dim);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .modules {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .hero {
    display: grid;
    grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
    gap: 12px;
    align-items: start;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .panel-head .head-sub {
    text-transform: none;
    letter-spacing: 0;
    color: var(--text-dim);
    font-weight: 500;
  }
  .head-right {
    margin-left: auto;
  }
  .pad0 {
    padding: 0;
  }
  .chart-lg {
    width: 100%;
    height: 290px;
  }
  .sub-label {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 0 2px;
    color: var(--text-dim);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th {
    text-align: left;
    padding: 5px 8px;
    border-bottom: 1px solid var(--border);
    color: var(--text-dim);
    font-weight: 500;
    white-space: nowrap;
    position: sticky;
    top: 0;
    background: var(--bg-panel);
  }
  td {
    padding: 5px 8px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  th.num,
  td.num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  th.ctr,
  td.ctr {
    text-align: center;
  }
  .dim {
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .tk {
    font-weight: 600;
    color: var(--accent);
  }
  .grp {
    font-weight: 600;
  }
  .vwap {
    color: #f5a623;
  }
  tr.anom {
    background: rgba(245, 166, 35, 0.07);
  }
  .alerts {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .alert {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    background: var(--bg-elev);
    border-radius: 5px;
    border-left: 3px solid var(--text-dim);
    font-size: 12px;
  }
  .a-time {
    width: 44px;
    color: var(--text-dim);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .a-tk {
    font-weight: 600;
    color: var(--accent);
    width: 56px;
  }
  .a-type {
    flex: none;
  }
  .a-metric {
    margin-left: auto;
    color: var(--text-dim);
  }
  .a-val {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  select {
    cursor: pointer;
  }
</style>
