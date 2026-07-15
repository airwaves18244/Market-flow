<script lang="ts">
  import { onDestroy } from "svelte";
  import Panel from "./Panel.svelte";
  import DatasetManager from "./DatasetManager.svelte";
  import { ipc, inTauri, onHistoryProgress, onHistoryDone, onHistoryError } from "../ipc";
  import type { AlgoMarket, DataSource, InstrumentDto, TimeRangeDto } from "../types";

  // Вкладка «Данные»: загрузка исторических данных + менеджер локальных датасетов.
  let { instruments = [] }: { instruments: InstrumentDto[] } = $props();

  const timeframes = ["m1", "m5", "m15", "h1", "d1"];
  // Рынки ALGOPACK — по образцу сегментов MoexAlgoTab; актуальны только для
  // источника «MOEX ALGO».
  const markets: { id: AlgoMarket; label: string }[] = [
    { id: "eq", label: "Акции" },
    { id: "fo", label: "Фьючерсы" },
    { id: "fx", label: "Валюта" },
  ];
  let source = $state<DataSource>("finam");
  let market = $state<AlgoMarket>("eq");
  let ticker = $state("SBER@MISX");
  let selectedTf = $state<Record<string, boolean>>({ d1: true, h1: false, m5: false });
  let fromDate = $state("2024-01-01");
  let tillDate = $state("2024-12-31");

  // Прогресс загрузки: в мок-режиме (браузер) — детерминированная симуляция,
  // в Tauri — реальные события `history:progress/done/error` от загрузчика.
  type Job = { key: string; label: string; pct: number };
  let jobs = $state<Job[]>([]);
  let plan = $state<TimeRangeDto[] | null>(null);
  let running = $state(false);
  let status = $state<string | null>(null);
  let manager: DatasetManager;

  // Активный id задачи (Tauri) и флаг отмены симуляции (браузер).
  let activeTaskId: number | null = null;
  let cancelSim = false;

  const toTs = (d: string) => Math.floor(new Date(d).getTime() / 1000);
  const jobKey = (tk: string, tf: string) => `${tk}-${tf}`;
  const jobLabel = (tk: string, tf: string) => `${tk} · ${tf.toUpperCase()}`;

  function setJobPct(key: string, pct: number) {
    const job = jobs.find((j) => j.key === key);
    if (job) {
      job.pct = pct;
      jobs = [...jobs];
    }
  }

  // Подписки на события загрузчика (только в Tauri; в браузере — no-op).
  let unlisten: Array<() => void> = [];
  async function subscribe() {
    if (!inTauri() || unlisten.length > 0) return;
    unlisten = await Promise.all([
      onHistoryProgress((p) => setJobPct(jobKey(p.ticker, p.tf), p.percent)),
      onHistoryDone((d) => {
        if (d.ticker === null) {
          // Итоговое событие всей загрузки.
          running = false;
          status = d.summary;
          void manager?.reload();
        }
      }),
      onHistoryError((e) => {
        status = e.ticker ? `Ошибка ${e.ticker}: ${e.message}` : e.message;
        // Терминальная ошибка всей загрузки (`ticker` = null) приходит, когда
        // источник не удалось стартовать — за ней события `history:done` не
        // будет, поэтому сбрасываем running здесь. Ошибка отдельной задачи
        // (`ticker` задан) не завершает загрузку.
        if (e.ticker === null) running = false;
      }),
    ]);
  }
  onDestroy(() => {
    for (const off of unlisten) off();
  });

  async function load() {
    if (running) return;
    const tfs = timeframes.filter((tf) => selectedTf[tf]);
    if (tfs.length === 0) return;

    status = null;
    running = true;
    cancelSim = false;
    activeTaskId = null;
    jobs = tfs.map((tf) => ({ key: jobKey(ticker, tf), label: jobLabel(ticker, tf), pct: 0 }));

    // Весь путь под try/catch: любая ошибка (план/старт задачи/симуляция) не
    // должна оставить кнопку «Загрузить» заблокированной (running=true).
    try {
      // План дозагрузки (какие диапазоны недостают) — реальный backend-вызов.
      plan = await ipc.historyPlan({
        covered: [],
        requestedFrom: toTs(fromDate),
        requestedTill: toTs(tillDate),
      });

      if (inTauri()) {
        // Боевой режим: подписываемся на события и стартуем фоновую загрузку.
        // Команда возвращается сразу с taskId; прогресс/итог/ошибка приходят
        // событиями `history:*`, поэтому running сбрасываем НЕ здесь, а в
        // обработчиках history:done / history:error (ticker=null).
        await subscribe();
        const task = await ipc.historyLoad({
          source,
          tickers: [ticker],
          timeframes: tfs,
          from: toTs(fromDate),
          till: toTs(tillDate),
          // Рынок актуален только для ALGOPACK; для finam бэкенд его игнорирует.
          ...(source === "moex_algo" ? { market } : {}),
        });
        activeTaskId = task.taskId;
      } else {
        // Браузер: детерминированная симуляция прогресса по каждому (тикер × ТФ).
        for (const job of jobs) {
          for (let p = 0; p <= 100; p += 20) {
            if (cancelSim) break;
            setJobPct(job.key, p);
            await new Promise((r) => setTimeout(r, 40));
          }
          if (cancelSim) break;
        }
        running = false;
        status = cancelSim ? "Загрузка отменена" : "Загрузка завершена";
        await manager?.reload();
      }
    } catch (e) {
      // Ошибка ДО запуска фоновой задачи (или в мок-режиме): гарантированно
      // разблокируем кнопку и показываем причину в статусе.
      running = false;
      status = String(e);
    }
  }

  async function cancel() {
    if (!running) return;
    if (inTauri()) {
      await ipc.historyCancel(activeTaskId ?? undefined);
    } else {
      cancelSim = true;
    }
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
      {#if source === "moex_algo"}
        <div class="tfs">
          <span class="lbl">Рынок</span>
          <div class="seg">
            {#each markets as mk (mk.id)}
              <button
                type="button"
                class="seg-btn"
                class:active={mk.id === market}
                onclick={() => (market = mk.id)}>{mk.label}</button
              >
            {/each}
          </div>
        </div>
      {/if}
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
      <button onclick={load} disabled={running}>Загрузить</button>
      {#if running}
        <button class="cancel" onclick={cancel}>Отменить</button>
      {/if}
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
    {#if status}
      <div class="status">{status}</div>
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
  .seg {
    display: flex;
    gap: 4px;
    margin-top: 3px;
  }
  .seg-btn {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
  }
  .seg-btn.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
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
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  button.cancel {
    background: transparent;
    border: 1px solid var(--border);
    color: #f5646c;
  }
  .status {
    margin-top: 8px;
    font-size: 12px;
    color: var(--text-dim);
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
