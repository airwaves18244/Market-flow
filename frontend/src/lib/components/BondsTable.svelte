<script lang="ts">
  import { fmtFixed } from "../format";
  import type { BondIssuerDto } from "../types";

  let { issuers = [] }: { issuers: BondIssuerDto[] } = $props();

  // Доходность/дюрация приходят 0, пока нет источника купонов/погашения
  // (см. api::bonds_rollup). Показываем «—» вместо ложного 0.00.
  function fmt(v: number): string {
    return v > 0 ? fmtFixed(v, 2) : "—";
  }
</script>

<table>
  <thead>
    <tr>
      <th>Эмитент</th>
      <th class="num">Выпусков</th>
      <th class="num">Оборот</th>
      <th class="num">Доходн.</th>
      <th class="num">Дюрация</th>
    </tr>
  </thead>
  <tbody>
    {#each issuers as issuer (issuer.issuer)}
      <tr>
        <td class="sym">{issuer.issuer}</td>
        <td class="num">{issuer.bonds}</td>
        <td class="num">{(issuer.turnover / 1_000_000).toFixed(1)}M</td>
        <td class="num">{fmt(issuer.avgYield)}</td>
        <td class="num">{fmt(issuer.weightedDuration)}</td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  tbody tr {
    cursor: default;
  }
  .sym {
    font-weight: 600;
    color: var(--accent);
  }
</style>
