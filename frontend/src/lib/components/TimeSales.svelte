<script lang="ts">
  import type { TimeAndSalesDto } from "../types";

  let { tape = null }: { tape: TimeAndSalesDto | null } = $props();

  function fmt(n: number): string {
    return n.toLocaleString("ru-RU", { maximumFractionDigits: 2 });
  }

  // Лента: новейшие сверху.
  let rows = $derived(tape ? [...tape.entries].reverse() : []);
</script>

{#if tape}
  <div class="ts">
    <div class="stats">
      <span>Сделок: {tape.stats.trades}</span>
      <span class:pos={tape.stats.cvd >= 0} class:neg={tape.stats.cvd < 0}>
        CVD: {tape.stats.cvd >= 0 ? "+" : ""}{fmt(tape.stats.cvd)}
      </span>
      <span>VWAP: {tape.stats.vwap != null ? fmt(tape.stats.vwap) : "—"}</span>
    </div>
    <div class="bar">
      <span
        class="buy"
        style="flex: {tape.stats.buyVolume || 1}"
        title="Покупки: {fmt(tape.stats.buyVolume)}"
      ></span>
      <span
        class="sell"
        style="flex: {tape.stats.sellVolume || 1}"
        title="Продажи: {fmt(tape.stats.sellVolume)}"
      ></span>
    </div>
    <table>
      <thead>
        <tr>
          <th class="num">Цена</th>
          <th class="num">Объём</th>
          <th>Сторона</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as e (e.ts)}
          <tr class:buy={e.side === "buy"} class:sell={e.side === "sell"}>
            <td class="num">{fmt(e.price)}</td>
            <td class="num">{fmt(e.size)}</td>
            <td>{e.side === "buy" ? "покупка" : "продажа"}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <div class="empty">Нет ленты сделок</div>
{/if}

<style>
  .ts {
    display: flex;
    flex-direction: column;
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .stats {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 2px 4px 6px;
    color: var(--text-dim);
    font-size: 11px;
  }
  .stats .pos {
    color: var(--up, #3fb950);
  }
  .stats .neg {
    color: var(--down, #f85149);
  }
  .bar {
    display: flex;
    height: 6px;
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 6px;
  }
  .bar .buy {
    background: var(--up, #3fb950);
  }
  .bar .sell {
    background: var(--down, #f85149);
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    padding: 1px 6px;
    text-align: left;
  }
  .num {
    text-align: right;
  }
  tbody tr {
    cursor: default;
  }
  tbody tr.buy td:first-child {
    color: var(--up, #3fb950);
    font-weight: 600;
  }
  tbody tr.sell td:first-child {
    color: var(--down, #f85149);
    font-weight: 600;
  }
  .empty {
    padding: 16px;
    color: var(--text-dim);
    text-align: center;
  }
</style>
