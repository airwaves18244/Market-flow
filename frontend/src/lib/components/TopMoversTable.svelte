<script lang="ts">
  import type { TopMoverDto } from "../types";

  export let movers: TopMoverDto[];

  function changeColor(change: number): string {
    return change > 0 ? "var(--up, #26a69a)" : "var(--down, #ef5350)";
  }

  function changeStr(change: number): string {
    const sign = change > 0 ? "+" : "";
    return sign + (change * 100).toFixed(2) + "%";
  }
</script>

<div class="movers-table">
  <h3>Top Movers</h3>
  <table>
    <thead>
      <tr>
        <th>Symbol</th>
        <th>Name</th>
        <th>Sector</th>
        <th>Change</th>
        <th>Price</th>
      </tr>
    </thead>
    <tbody>
      {#each movers as mover (mover.symbol)}
        <tr>
          <td class="symbol">{mover.ticker}</td>
          <td class="name">{mover.name}</td>
          <td class="sector">{mover.sector ?? "Other"}</td>
          <td class="change" style="color: {changeColor(mover.change)}">
            {changeStr(mover.change)}
          </td>
          <td class="price">{mover.lastClose.toFixed(2)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .movers-table {
    padding: 12px;
    background: var(--bg-secondary, #161b22);
    border-radius: 6px;
    border: 1px solid var(--border, #30363d);
  }

  h3 {
    margin: 0 0 12px 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #c9d1d9);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  th {
    background: var(--bg-tertiary, #0d1117);
    color: var(--text-secondary, #8b949e);
    padding: 6px 8px;
    text-align: left;
    font-weight: 600;
    border-bottom: 1px solid var(--border, #30363d);
  }

  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--border, #30363d);
    color: var(--text-primary, #c9d1d9);
  }

  .symbol {
    font-weight: 600;
    color: var(--accent, #4f9cf9);
  }

  .name {
    font-size: 11px;
    color: var(--text-secondary, #8b949e);
  }

  .sector {
    font-size: 11px;
    color: var(--text-secondary, #8b949e);
  }

  .change {
    font-weight: 600;
  }

  .price {
    text-align: right;
    font-family: monospace;
  }

  tbody tr:hover {
    background: var(--bg-tertiary, #0d1117);
  }
</style>
