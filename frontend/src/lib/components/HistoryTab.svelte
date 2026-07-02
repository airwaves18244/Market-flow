<script lang="ts">
  import Panel from "./Panel.svelte";
  import DatasetManager from "./DatasetManager.svelte";
  import { ipc } from "../ipc";
  import type { DataSource, InstrumentDto, TimeRangeDto } from "../types";

  // Вкладка «Данные»: загрузка исторических данных + менеджер локальных датасетов.
  let { instruments = [] }: { instruments: InstrumentDto[] } = $props();

  const timeframes = ["m1", "m5", "m15", "h1", "d1"];
  let source = $state<DataSource>("finam");
  let ticker = $state("SBER@MISX");
  let selectedTf = $state<Record<string, boolean>>({ d1: true, h1: false, m5: false });
  let fromDate = $state("2024-01-01");
  let tillDate = $state("2024-12-31");

  // Прогресс загрузки (в мок-режиме — симуляция; в бою — события `history:progress`).
  type Job = { key: string; label: string; pct: number };
  let jobs = $state<Job[]>([]);
  let plan = $state<TimeRangeDto[] | null>(null);
  let manager: DatasetManager;

  const toTs = (d: string) => Math.floor(new Date(d).getTime() / 1000);

  async function load() {
    const tfs = timeframes.filter((tf) => selectedTf[tf]);
    if (tfs.length === 0) return;

    // План дозагрузки (какие диапазоны недостают) — реальный backend-вызов.
    plan = await ipc.historyPlan({
      covered: [],
      requestedFrom: toTs(fromDate),
      requestedTill: toTs(tillDate),
    });

    // Симулируем прогресс по каждому (тикер × ТФ). В боевом режиме прогресс
    // приходит событиями `history:progress/done/error`.
    jobs = tfs.map((tf) => ({ key: `${ticker}-${tf}`, label: `${ticker} · ${tf.toUpperCase()}`, pct: 0 }));
    for (const job of jobs) {
      for (let p = 0; p <= 100; p += 20) {
        job.pct = p;
        jobs = [...jobs];
        await new Promise((r) => setTimeout(r, 40));
      }
    }
    await manager?.reload();
  }
</script>

<div class="history">
  <Panel title="Загрузка истории">
    <div class="form">
      <label>
        Источник
        <select bind:value={source}>
          <option value="finam">Finam Trade API</option>
          <option value="moex_algo">MOEX ALGO</option>
        </select>
      </label>
      <label>
        Инструмент
        <select bind:value={ticker}>
          {#each instruments as i (i.symbol)}
            <option value={i.symbol}>{i.ticker}</option>
          {/each}
          {#if instruments.length === 0}
            <option value="SBER@MISX">SBER</option>
          {/if}
        </select>
      </label>
      <div class="tfs">
        <span class="lbl">Таймфреймы</span>
        <div class="chips">
          {#each timeframes as tf (tf)}
            <label class="chip">
              <input type="checkbox" bind:checked={selectedTf[tf]} />
              {tf.toUpperCase()}
            </label>
          {/each}
        </div>
      </div>
      <label>С<input type="date" bind:value={fromDate} /></label>
      <label>По<input type="date" bind:value={tillDate} /></label>
      <button onclick={load}>Загрузить</button>
    </div>

    {#if plan}
      <div class="plan">Дозагрузка: {plan.length} диапазон(ов) недостаёт в локальном хранилище.</div>
    {/if}
    {#if jobs.length > 0}
      <div class="jobs">
        {#each jobs as job (job.key)}
          <div class="job">
            <span class="jlabel">{job.label}</span>
            <div class="bar"><div class="fill" style="width:{job.pct}%"></div></div>
            <span class="jpct">{job.pct}%</span>
          </div>
        {/each}
      </div>
    {/if}
  </Panel>

  <DatasetManager bind:this={manager} />
</div>

<style>
  .history {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
  }
  .form {
    display: flex;
    gap: 12px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    flex-direction: column;
    font-size: 11px;
    color: var(--text-dim);
    gap: 3px;
  }
  select,
  input[type="date"] {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    padding: 4px 6px;
    font-size: 12px;
  }
  .tfs .lbl {
    font-size: 11px;
    color: var(--text-dim);
  }
  .chips {
    display: flex;
    gap: 4px;
    margin-top: 3px;
  }
  .chip {
    flex-direction: row;
    align-items: center;
    gap: 3px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 3px 6px;
    font-size: 11px;
    color: var(--text);
  }
  button {
    background: var(--accent);
    border: none;
    border-radius: 4px;
    color: #fff;
    padding: 7px 16px;
    font-size: 12px;
    cursor: pointer;
  }
  .plan {
    margin-top: 8px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .jobs {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .job {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
  }
  .jlabel {
    width: 120px;
    color: var(--text-dim);
  }
  .bar {
    flex: 1;
    height: 6px;
    background: var(--bg-elev);
    border-radius: 3px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.1s;
  }
  .jpct {
    width: 36px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
</style>
