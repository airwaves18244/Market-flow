<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Panel from "./Panel.svelte";
  import EquityCurveChart from "./EquityCurveChart.svelte";
  import { ipc } from "../ipc";
  import { fmtRu } from "../format";
  import type {
    BacktestReportDto,
    InstrumentDto,
    StrategyDescriptorDto,
  } from "../types";

  let { instruments, selected }: { instruments: InstrumentDto[]; selected: string } = $props();

  const FULL_RANGE = Number.MAX_SAFE_INTEGER;

  let strategies = $state<StrategyDescriptorDto[]>([]);
  let strategyId = $state("ma_cross");
  let params = $state<Record<string, number>>({});
  let initialCapital = $state(100_000);
  let commission = $state(0);
  let slippage = $state(0);
  let report = $state<BacktestReportDto | null>(null);
  let running = $state(false);
  let error = $state<string | null>(null);

  const current = $derived(strategies.find((s) => s.id === strategyId));

  // При смене стратегии заполняем параметры значениями по умолчанию.
  // Зависим только от strategyId/strategies; чтение/запись params — вне трекинга,
  // иначе эффект зациклится (читает и пишет один и тот же стейт).
  $effect(() => {
    const id = strategyId;
    const desc = strategies.find((s) => s.id === id);
    if (!desc) return;
    untrack(() => {
      const next: Record<string, number> = {};
      for (const p of desc.params) next[p.name] = params[p.name] ?? p.default;
      params = next;
    });
  });

  async function run() {
    running = true;
    error = null;
    try {
      report = await ipc.runBacktest(selected, "d1", 0, FULL_RANGE, strategyId, params, {
        initialCapital,
        commission,
        slippage,
        fillTiming: "nextOpen",
      });
    } catch (e) {
      error = String(e);
      report = null;
    } finally {
      running = false;
    }
  }

  onMount(async () => {
    try {
      strategies = await ipc.listStrategies();
    } catch (e) {
      error = String(e);
    }
  });

  const pct = (n: number) => `${(n * 100).toFixed(2)}%`;
  const pf = (n: number) => (Number.isFinite(n) ? n.toFixed(2) : "∞");
</script>

<main class="grid">
  <Panel title="Бэктест — параметры">
    <div class="form">
      <div class="field">
        <span>Инструмент</span>
        <strong>{selected}</strong>
      </div>
      <label>
        Стратегия
        <select bind:value={strategyId}>
          {#each strategies as s (s.id)}
            <option value={s.id}>{s.label}</option>
          {/each}
        </select>
      </label>

      {#if current}
        {#each current.params as p (p.name)}
          <label>
            {p.label}
            <input type="number" bind:value={params[p.name]} step="any" />
          </label>
        {/each}
      {/if}

      <label>
        Стартовый капитал
        <input type="number" bind:value={initialCapital} step="any" />
      </label>
      <label>
        Комиссия (за ед.)
        <input type="number" bind:value={commission} step="any" />
      </label>
      <label>
        Проскальзывание
        <input type="number" bind:value={slippage} step="any" />
      </label>

      <button class="run" onclick={run} disabled={running}>
        {running ? "Считаю…" : "Запустить бэктест"}
      </button>
      {#if error}<div class="error">{error}</div>{/if}
    </div>
  </Panel>

  <Panel title="Кривая капитала">
    {#if report && report.equityCurve.length}
      <EquityCurveChart points={report.equityCurve} />
    {:else}
      <p class="empty">Запустите бэктест, чтобы увидеть кривую капитала.</p>
    {/if}
  </Panel>

  {#if report}
    <Panel title="Метрики">
      <div class="metrics">
        <div class="metric" class:up={report.metrics.netPnl >= 0} class:down={report.metrics.netPnl < 0}>
          <span>Чистый P&L</span><b>{fmtRu(report.metrics.netPnl)}</b>
        </div>
        <div class="metric"><span>Доходность</span><b>{pct(report.metrics.returnPct)}</b></div>
        <div class="metric"><span>Сделок</span><b>{report.metrics.trades}</b></div>
        <div class="metric"><span>Win-rate</span><b>{pct(report.metrics.winRate)}</b></div>
        <div class="metric"><span>Profit factor</span><b>{pf(report.metrics.profitFactor)}</b></div>
        <div class="metric"><span>Макс. просадка</span><b>{pct(report.metrics.maxDrawdown)}</b></div>
        <div class="metric"><span>Sharpe</span><b>{report.metrics.sharpe.toFixed(2)}</b></div>
        <div class="metric"><span>Ср. прибыль</span><b>{fmtRu(report.metrics.avgWin)}</b></div>
        <div class="metric"><span>Ср. убыток</span><b>{fmtRu(report.metrics.avgLoss)}</b></div>
      </div>
    </Panel>

    <Panel title={`Сделки (${report.trades.length})`}>
      <table>
        <thead>
          <tr><th>Время</th><th>Сторона</th><th class="num">Кол-во</th><th class="num">Цена</th><th class="num">P&L</th></tr>
        </thead>
        <tbody>
          {#each report.trades as t (t.ts + t.side + t.price)}
            <tr>
              <td>{new Date(t.ts * 1000).toLocaleDateString("ru-RU")}</td>
              <td class:up={t.side === "buy"} class:down={t.side === "sell"}>{t.side === "buy" ? "Покупка" : "Продажа"}</td>
              <td class="num">{fmtRu(t.qty)}</td>
              <td class="num">{fmtRu(t.price)}</td>
              <td class="num" class:up={t.realizedPnl > 0} class:down={t.realizedPnl < 0}>{t.realizedPnl ? fmtRu(t.realizedPnl) : "—"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </Panel>
  {/if}
</main>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .form label,
  .form .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .form input,
  .form select {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text);
    padding: 5px 8px;
    font-size: 13px;
  }
  .form strong {
    color: var(--text);
    font-size: 13px;
  }
  .run {
    margin-top: 6px;
    background: var(--accent);
    color: #06101f;
    border: none;
    border-radius: 6px;
    padding: 8px;
    font-weight: 600;
    cursor: pointer;
  }
  .run:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .metrics {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 8px;
  }
  .metric {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-elev);
  }
  .metric span {
    font-size: 11px;
    color: var(--text-dim);
  }
  .metric b {
    font-size: 15px;
    font-variant-numeric: tabular-nums;
  }
  .empty {
    color: var(--text-dim);
    font-size: 13px;
    padding: 8px;
  }
</style>
