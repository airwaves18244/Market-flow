<script lang="ts">
  import Panel from "./Panel.svelte";
  import type { KeyActivityPeriod, KeyActivityRowDto } from "../types";

  // Таблица «Ключевая активность» с фильтрами по правилу/тикеру.
  let {
    rows = [],
    period = "1h",
    onPeriod,
  }: {
    rows: KeyActivityRowDto[];
    period: KeyActivityPeriod;
    onPeriod: (p: KeyActivityPeriod) => void;
  } = $props();

  const periods: KeyActivityPeriod[] = ["1h", "1d", "1w", "1m", "3m"];
  let ruleFilter = $state("");
  let tickerFilter = $state("");

  const rules = $derived([...new Set(rows.map((r) => r.ruleName))]);
  const filtered = $derived(
    rows.filter(
      (r) =>
        (!ruleFilter || r.ruleName === ruleFilter) &&
        (!tickerFilter || r.secid.toLowerCase().includes(tickerFilter.toLowerCase())),
    ),
  );

  const sev = (imp: number) => (imp >= 0.9 ? "high" : imp >= 0.7 ? "mid" : "low");
</script>

<Panel title="Ключевая активность">
  <div class="ka">
    <div class="toolbar">
      <div class="periods">
        {#each periods as p (p)}
          <button class="p" class:active={p === period} onclick={() => onPeriod(p)}>{p}</button>
        {/each}
      </div>
      <select bind:value={ruleFilter}>
        <option value="">все правила</option>
        {#each rules as r (r)}
          <option value={r}>{r}</option>
        {/each}
      </select>
      <input placeholder="тикер…" bind:value={tickerFilter} />
    </div>

    {#if filtered.length === 0}
      <div class="empty">нет данных</div>
    {:else}
      <table>
        <thead>
          <tr><th>тикер</th><th>правило</th><th>метрика</th><th>значение</th><th>важность</th></tr>
        </thead>
        <tbody>
          {#each filtered as r (r.secid + r.ruleId)}
            <tr>
              <td class="tk">{r.secid}</td>
              <td><span class="chip {sev(r.importance)}">{r.ruleName}</span></td>
              <td>{r.metric}</td>
              <td class="num">{r.value.toFixed(3)}</td>
              <td class="num">{r.importance.toFixed(2)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</Panel>

<style>
  .ka {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .toolbar {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .periods {
    display: flex;
    gap: 2px;
  }
  .p {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 4px;
    font-size: 11px;
    padding: 4px 8px;
    cursor: pointer;
  }
  .p.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  select,
  input {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    font-size: 12px;
    padding: 4px 6px;
  }
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
  .chip {
    display: inline-block;
    padding: 1px 8px;
    border-radius: 10px;
    font-size: 11px;
  }
  .chip.high {
    background: rgba(245, 100, 108, 0.18);
    color: #f5646c;
  }
  .chip.mid {
    background: rgba(245, 166, 35, 0.18);
    color: #f5a623;
  }
  .chip.low {
    background: rgba(79, 156, 249, 0.18);
    color: #4f9cf9;
  }
  .empty {
    font-size: 12px;
    color: var(--text-dim);
    padding: 12px;
  }
</style>
