<script lang="ts">
  import type { BreadthDto } from "../types";

  let { data }: { data: BreadthDto } = $props();

  const pctStr = $derived(
    data.pctAdvancing != null ? `${(data.pctAdvancing * 100).toFixed(1)}%` : "—",
  );
  const adStr = $derived(data.adRatio != null ? data.adRatio.toFixed(2) : "—");
</script>

<div class="metrics">
  <div class="metric">
    <div class="label">Растущие</div>
    <div class="value advancers">{data.advancers}</div>
  </div>
  <div class="metric">
    <div class="label">Падающие</div>
    <div class="value decliners">{data.decliners}</div>
  </div>
  <div class="metric">
    <div class="label">Без изм.</div>
    <div class="value">{data.unchanged}</div>
  </div>
  <div class="metric">
    <div class="label">% растущих</div>
    <div class="value">{pctStr}</div>
  </div>
  <div class="metric">
    <div class="label">A/D ratio</div>
    <div class="value">{adStr}</div>
  </div>
</div>

<style>
  .metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(72px, 1fr));
    gap: 8px;
  }

  .metric {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .label {
    font-size: 11px;
    color: var(--text-dim);
    text-transform: uppercase;
  }

  .value {
    font-size: 18px;
    font-weight: 700;
    color: var(--text);
  }

  .value.advancers {
    color: var(--up);
  }

  .value.decliners {
    color: var(--down);
  }
</style>
