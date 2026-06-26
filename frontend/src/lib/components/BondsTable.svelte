<script lang="ts">
  import type { BondIssuerDto } from "../types";

  export let issuers: BondIssuerDto[];
</script>

<div class="bonds-table">
  <h3>Bond Issuers</h3>
  <table>
    <thead>
      <tr>
        <th>Issuer</th>
        <th>Bonds</th>
        <th>Turnover</th>
        <th>Yield %</th>
        <th>Duration</th>
      </tr>
    </thead>
    <tbody>
      {#each issuers as issuer (issuer.issuer)}
        <tr>
          <td class="issuer">{issuer.issuer}</td>
          <td class="count">{issuer.bonds}</td>
          <td class="turnover">{(issuer.turnover / 1_000_000).toFixed(1)}M</td>
          <td class="yield">{issuer.avgYield.toFixed(2)}</td>
          <td class="duration">{issuer.weightedDuration.toFixed(2)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .bonds-table {
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

  .issuer {
    font-weight: 600;
    color: var(--accent, #4f9cf9);
  }

  .count {
    text-align: center;
  }

  .turnover {
    text-align: right;
    font-family: monospace;
  }

  .yield {
    text-align: right;
    font-family: monospace;
  }

  .duration {
    text-align: right;
    font-family: monospace;
  }

  tbody tr:hover {
    background: var(--bg-tertiary, #0d1117);
  }
</style>
