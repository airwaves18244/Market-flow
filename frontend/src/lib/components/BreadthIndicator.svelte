<script lang="ts">
  import type { BreadthDto } from "../types";

  export let data: BreadthDto;

  const pctStr = data.pctAdvancing
    ? `${(data.pctAdvancing * 100).toFixed(1)}%`
    : "N/A";
  const adStr = data.adRatio ? data.adRatio.toFixed(2) : "N/A";
</script>

<div class="breadth-indicator">
  <h3>Market Breadth</h3>
  <div class="metrics">
    <div class="metric">
      <div class="label">Advancers</div>
      <div class="value advancers">{data.advancers}</div>
    </div>
    <div class="metric">
      <div class="label">Decliners</div>
      <div class="value decliners">{data.decliners}</div>
    </div>
    <div class="metric">
      <div class="label">Unchanged</div>
      <div class="value">{data.unchanged}</div>
    </div>
    <div class="metric">
      <div class="label">% Advancing</div>
      <div class="value">{pctStr}</div>
    </div>
    <div class="metric">
      <div class="label">A/D Ratio</div>
      <div class="value">{adStr}</div>
    </div>
  </div>
</div>

<style>
  .breadth-indicator {
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

  .metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(80px, 1fr));
    gap: 8px;
  }

  .metric {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .label {
    font-size: 11px;
    color: var(--text-secondary, #8b949e);
    text-transform: uppercase;
  }

  .value {
    font-size: 18px;
    font-weight: 700;
    color: var(--text-primary, #c9d1d9);
  }

  .value.advancers {
    color: var(--up, #26a69a);
  }

  .value.decliners {
    color: var(--down, #ef5350);
  }
</style>
