<script lang="ts">
  import { onMount } from "svelte";
  import KeyActivityTable from "./KeyActivityTable.svelte";
  import KeyActivitySummary from "./KeyActivitySummary.svelte";
  import SuperCandlesChart from "./SuperCandlesChart.svelte";
  import DisbBars from "./DisbBars.svelte";
  import FutoiChart from "./FutoiChart.svelte";
  import Hi2Chart from "./Hi2Chart.svelte";
  import { ipc } from "../ipc";
  import { loadSettings } from "../settings";
  import * as algo from "../algoMock";
  import type {
    KeyActivityPeriod,
    KeyActivityRowDto,
    KeyActivitySampleInput,
    KeyActivitySummaryDto,
  } from "../types";

  // Вкладка «MOEX ALGO». Герой вкладки — «Ключевая активность» + «ИТОГО»
  // (задачи 10.7.6/10.7.7) на боевом IPC (`key_activity`). Остальные модули
  // (Супер-свечи, FUTOI, HI2, Мега-алёрты) визуализируют аналитику
  // `domain::algo`; ряды пока идут из детерминированных демо-генераторов
  // (`algoMock`) до подключения боевого транспорта ALGOPACK (`data::moex`).

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

  // Селектор рынка. В демо-режиме генераторы не сегментированы по рынку, поэтому
  // переключатель пока только визуальный (как в дизайн-прототипе); scope по
  // рынку появится вместе с боевым транспортом ALGOPACK (`data::moex`).
  const markets = [
    { id: "eq", label: "Акции" },
    { id: "fo", label: "Фьючерсы" },
    { id: "fx", label: "Валюта" },
  ] as const;
  let market = $state<(typeof markets)[number]["id"]>("eq");

  const universe = algo.tickers();
  let ticker = $state("SBER");
  let futoiMode = $state<"long" | "short" | "net">("net");
  let megaType = $state("all");

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

  onMount(async () => {
    await loadRows();
    await loadSummary();
  });

  // ── Производные ряды для остальных модулей (демо-генераторы) ────────────────
  const bars = $derived(algo.candles(ticker));
  const scRows = $derived(bars.slice(-14).reverse());
  const futoi = $derived(algo.futoiSeries(ticker));
  const futoiRows = $derived(algo.futoiTable(ticker));
  const hi2 = $derived(algo.hi2Timeline(ticker));
  const hi2Rank = $derived(algo.hi2Ranking());
  const mega = $derived(algo.megaAlerts());
  const megaTypes = $derived(["all", ...new Set(mega.map((a) => a.type))]);
  const megaView = $derived(mega.filter((a) => megaType === "all" || a.type === megaType));

  const sevLabel = (s: string) => (s === "high" ? "высокая" : s === "med" ? "средняя" : "низкая");
  const hi2Level = (v: number) =>
    v > 0.3 ? "доминирование" : v > 0.18 ? "умеренная" : "распределённая";
  const hi2Color = (v: number) => (v > 0.3 ? "var(--down)" : v > 0.18 ? "#f5a623" : "var(--up)");
</script>

<div class="algo">
  <!-- Тулбар: инструмент · период · рынок -->
  <div class="toolbar">
    <span class="tb-label">Инструмент</span>
    <select class="ctl-sm" bind:value={ticker}>
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
        <button class="seg-btn" class:active={mk.id === market} onclick={() => (market = mk.id)}>
          {mk.label}
        </button>
      {/each}
    </div>
    <span class="status-line">{universe.length} инструментов · {ticker} · демо-режим</span>
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
          <div class="chart-lg"><SuperCandlesChart {bars} /></div>
          <div class="sub-label">Дисбаланс покупок/продаж</div>
          <DisbBars {bars} />
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
              {#each scRows as b (b.min)}
                <tr class:anom={b.anomalous}>
                  <td class="dim">{algo.hhmm(b.min)}</td>
                  <td class="num">{algo.fmt(b.o)}</td><td class="num">{algo.fmt(b.h)}</td>
                  <td class="num">{algo.fmt(b.l)}</td><td class="num">{algo.fmt(b.c)}</td>
                  <td class="num vwap">{algo.fmt(b.vwap)}</td><td class="num">{algo.fmtInt(b.vol)}</td>
                  <td class="num">{algo.fmtInt(b.trades)}</td>
                  <td class="num" class:up={b.disb >= 0} class:down={b.disb < 0}>
                    {(b.disb >= 0 ? "+" : "") + algo.fmt(b.disb, 2)}
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
        <div class="panel-body"><FutoiChart series={futoi} mode={futoiMode} /></div>
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
                  <td class="num">{algo.fmtInt(r.long)}</td>
                  <td class="num">{algo.fmtInt(r.short)}</td>
                  <td class="num" class:up={r.net >= 0} class:down={r.net < 0}>
                    {(r.net >= 0 ? "+" : "−") + algo.fmtInt(Math.abs(r.net))}
                  </td>
                  <td class="num">{algo.fmt(r.sharePct, 1)}%</td>
                  <td class="num" class:up={r.doi >= 0} class:down={r.doi < 0}>
                    {(r.doi >= 0 ? "+" : "−") + algo.fmtInt(Math.abs(r.doi))}
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
        <div class="panel-body"><Hi2Chart times={hi2.times} vals={hi2.vals} /></div>
      </section>
      <section class="panel">
        <header class="panel-head">Ранжирование по концентрации</header>
        <div class="panel-body pad0">
          <table>
            <thead>
              <tr><th>Тикер</th><th class="num">HI2</th><th>Уровень</th><th class="ctr">Всплеск</th></tr>
            </thead>
            <tbody>
              {#each hi2Rank as r (r.ticker)}
                <tr>
                  <td class="tk">{r.ticker}</td>
                  <td class="num">{algo.fmt(r.hi2, 3)}</td>
                  <td style:color={hi2Color(r.hi2)}>{hi2Level(r.hi2)}</td>
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
            <option value={t}>{t === "all" ? "Все типы" : t}</option>
          {/each}
        </select>
      </header>
      <div class="panel-body">
        <div class="alerts">
          {#each megaView as a, i (i)}
            <div
              class="alert"
              style:border-left-color={a.severity === "high"
                ? "var(--down)"
                : a.severity === "med"
                  ? "#f5a623"
                  : "var(--text-dim)"}
            >
              <span class="a-time">{a.time}</span>
              <span class="a-tk">{a.ticker}</span>
              <span class="a-type">{a.type}</span>
              <span class="a-metric">{a.metric}</span>
              <span class="a-val" class:up={a.up} class:down={!a.up}>{a.value}</span>
              <span class="sev {a.severity}">{sevLabel(a.severity)}</span>
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
