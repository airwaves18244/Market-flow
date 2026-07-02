<script lang="ts">
  import { onMount } from "svelte";
  import KeyActivityTable from "./KeyActivityTable.svelte";
  import KeyActivitySummary from "./KeyActivitySummary.svelte";
  import { ipc } from "../ipc";
  import type {
    KeyActivityPeriod,
    KeyActivityRowDto,
    KeyActivitySampleInput,
    KeyActivitySummaryDto,
  } from "../types";

  // Вкладка «MOEX ALGO». В центре — «Ключевая активность» + «ИТОГО» (герой
  // вкладки, задачи 10.7.6/10.7.7). Остальные модули (Super Candles, FUTOI, HI2,
  // Mega Alerts) подключаются с боевым транспортом MOEX ALGOPACK (`data::moex`).
  const modules = [
    { id: "key", label: "Ключевая активность" },
    { id: "super", label: "Супер-свечи" },
    { id: "futoi", label: "FUTOI" },
    { id: "hi2", label: "Концентрация HI2" },
    { id: "mega", label: "Мега-алёрты" },
  ];
  let active = $state("key");

  // Демонстрационный набор образцов метрик (в боевом режиме собирается из
  // датасетов ALGOPACK tradestats/futoi/hi2).
  const demoSamples: KeyActivitySampleInput[] = [
    { secid: "SBER", ts: 4, volume: 5200, volumeZ: 3.8, disb: 0.55, hi2: 0.22, priceChange: 0.031 },
    { secid: "GAZP", ts: 4, volume: 900, volumeZ: 0.6, disb: -0.62, hi2: 0.71, priceChange: -0.008 },
    { secid: "LKOH", ts: 4, volume: 2100, volumeZ: 1.2, disb: 0.15, hi2: 0.34, priceChange: 0.026 },
    { secid: "GMKN", ts: 4, volume: 1500, volumeZ: 2.9, disb: 0.05, hi2: 0.28, priceChange: -0.004 },
  ];

  let period = $state<KeyActivityPeriod>("1h");
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
    await loadSummary();
  }

  onMount(async () => {
    await loadRows();
    await loadSummary();
  });
</script>

<div class="algo">
  <nav class="subnav">
    {#each modules as m (m.id)}
      <button class="seg" class:active={m.id === active} onclick={() => (active = m.id)}>
        {m.label}
      </button>
    {/each}
  </nav>

  {#if active === "key"}
    <div class="hero">
      <KeyActivityTable {rows} {period} onPeriod={setPeriod} />
      <KeyActivitySummary {summary} loading={loadingSummary} onRefresh={loadSummary} />
    </div>
  {:else}
    <div class="placeholder">
      Модуль «{modules.find((m) => m.id === active)?.label}» подключается с боевым транспортом
      MOEX ALGOPACK (<code>data::moex</code>). Аналитика уже реализована в доменном ядре
      (<code>domain::algo</code>); здесь появится график и таблица после подключения источника.
    </div>
  {/if}
</div>

<style>
  .algo {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
  }
  .subnav {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .seg {
    appearance: none;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-dim);
    font-size: 12px;
    padding: 6px 12px;
    cursor: pointer;
  }
  .seg.active {
    color: #fff;
    background: var(--accent);
    border-color: var(--accent);
  }
  .hero {
    display: grid;
    grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
    gap: 12px;
    align-items: start;
  }
  .placeholder {
    font-size: 13px;
    color: var(--text-dim);
    line-height: 1.6;
    border: 1px dashed var(--border);
    border-radius: 8px;
    padding: 24px;
  }
  code {
    background: var(--bg-elev);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 12px;
  }
</style>
