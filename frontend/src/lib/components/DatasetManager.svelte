<script lang="ts">
  import { onMount } from "svelte";
  import Panel from "./Panel.svelte";
  import { ipc } from "../ipc";
  import type { DatasetMetaDto } from "../types";

  // Таблица локальных датасетов истории (источник/тикер/ТФ/диапазон/бары/…).
  let datasets = $state<DatasetMetaDto[]>([]);
  let error = $state<string | null>(null);

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
    await reload();
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
          <tr>
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
            <td><button class="rm" onclick={() => del(d)}>удалить</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</Panel>

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
</style>
