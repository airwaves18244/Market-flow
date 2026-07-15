<script lang="ts">
  import { onMount } from "svelte";
  import Panel from "./Panel.svelte";
  import CandleChart from "./CandleChart.svelte";
  import { ipc } from "../ipc";
  import type { BarPoint, DatasetMetaDto } from "../types";

  // Таблица локальных датасетов истории (источник/тикер/ТФ/диапазон/бары/…).
  let datasets = $state<DatasetMetaDto[]>([]);
  let error = $state<string | null>(null);

  // Превью датасета свечами (11.4.4): выбранный датасет + его последние бары.
  let preview = $state<DatasetMetaDto | null>(null);
  let previewBars = $state<BarPoint[]>([]);
  let previewError = $state<string | null>(null);

  const day = (ts: number) => new Date(ts * 1000).toISOString().slice(0, 10);
  const sizeKb = (bars: number) => `${Math.round((bars * 48) / 1024)} КБ`;
  const sourceLabel = (s: string) => (s === "finam" ? "Finam" : s === "moex_algo" ? "MOEX ALGO" : s);

  export async function reload() {
    try {
      datasets = await ipc.historyDatasets();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function del(d: DatasetMetaDto) {
    await ipc.historyDelete({ source: d.source, secid: d.secid, tf: d.tf });
    if (preview && preview.source === d.source && preview.secid === d.secid && preview.tf === d.tf) {
      closePreview();
    }
    await reload();
  }

  async function openPreview(d: DatasetMetaDto) {
    preview = d;
    previewError = null;
    previewBars = [];
    try {
      previewBars = await ipc.historyPreview(d.source, d.secid, d.tf, 300);
    } catch (e) {
      previewError = String(e);
    }
  }

  function closePreview() {
    preview = null;
    previewBars = [];
    previewError = null;
  }

  onMount(reload);
</script>

<Panel title="Локальные датасеты">
  {#if error}
    <div class="err">{error}</div>
  {:else if datasets.length === 0}
    <div class="empty">нет локальных датасетов — загрузите историю выше</div>
  {:else}
    <table>
      <thead>
        <tr>
          <th>источник</th><th>тикер</th><th>ТФ</th><th>диапазон</th>
          <th class="num">баров</th><th class="num">размер</th><th>обновлено</th><th></th>
        </tr>
      </thead>
      <tbody>
        {#each datasets as d (d.source + d.secid + d.tf)}
          <tr class="row" onclick={() => openPreview(d)} title="показать свечи">
            <td>{sourceLabel(d.source)}</td>
            <td class="tk">{d.secid}</td>
            <td>{d.tf.toUpperCase()}</td>
            <td>{day(d.fromTs)} — {day(d.toTs)}</td>
            <td class="num">{d.bars}</td>
            <td class="num">{sizeKb(d.bars)}</td>
            <td>
              {day(d.updatedTs)}
              {#if !d.looksComplete}<span class="gap" title="есть пропуски">◐</span>{/if}
            </td>
            <td>
              <button class="rm" onclick={(e) => { e.stopPropagation(); del(d); }}>удалить</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</Panel>

{#if preview}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="0"
    onclick={closePreview}
    onkeydown={(e) => e.key === "Escape" && closePreview()}
  >
    <div
      class="modal"
      role="dialog"
      tabindex="-1"
      aria-label="Превью датасета"
      onclick={(e) => e.stopPropagation()}
      onkeydown={() => {}}
    >
      <div class="modal-head">
        <span class="modal-title">
          {sourceLabel(preview.source)} · {preview.secid} · {preview.tf.toUpperCase()}
        </span>
        <button class="close" onclick={closePreview}>✕</button>
      </div>
      {#if previewError}
        <div class="pv-err">{previewError}</div>
      {:else if previewBars.length === 0}
        <div class="pv-empty">нет баров для превью</div>
      {:else}
        <div class="chart">
          <CandleChart bars={previewBars} />
        </div>
        <div class="pv-note">последние {previewBars.length} баров</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th {
    text-align: left;
    color: var(--text-dim);
    font-weight: 500;
    padding: 4px 6px;
    border-bottom: 1px solid var(--border);
  }
  td {
    padding: 4px 6px;
    border-bottom: 1px solid var(--border);
  }
  .row {
    cursor: pointer;
  }
  .row:hover {
    background: var(--bg-elev);
  }
  .tk {
    font-weight: 600;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .gap {
    color: #f5a623;
    margin-left: 4px;
  }
  .rm {
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: #f5646c;
    font-size: 11px;
    padding: 2px 8px;
    cursor: pointer;
  }
  .empty,
  .err {
    font-size: 12px;
    color: var(--text-dim);
    padding: 12px;
  }
  .err {
    color: #f5646c;
  }

  /* ── Превью датасета (модал со свечами) ──────────────────────────────────── */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .modal {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: min(760px, 92vw);
    padding: 12px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
  }
  .modal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .modal-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }
  .close {
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-dim);
    cursor: pointer;
    padding: 2px 8px;
    font-size: 12px;
  }
  .chart {
    height: 320px;
    width: 100%;
  }
  .pv-note {
    margin-top: 6px;
    font-size: 11px;
    color: var(--text-dim);
    text-align: right;
  }
  .pv-empty,
  .pv-err {
    font-size: 12px;
    color: var(--text-dim);
    padding: 24px;
    text-align: center;
  }
  .pv-err {
    color: #f5646c;
  }
</style>
