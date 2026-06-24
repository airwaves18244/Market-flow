<script lang="ts">
  import { store } from "../store.svelte";

  const movers = $derived(store.equity?.top_movers ?? []);
  const pct = (x: number) => `${(x * 100).toFixed(2)}%`;
  const money = (x: number) =>
    new Intl.NumberFormat("ru-RU", { notation: "compact", maximumFractionDigits: 1 }).format(x);
</script>

<div class="table-wrap">
  <table>
    <thead>
      <tr>
        <th>Тикер</th>
        <th>Инструмент</th>
        <th class="num">Δ</th>
        <th class="num">Оборот</th>
      </tr>
    </thead>
    <tbody>
      {#each movers as m (m.symbol)}
        <tr>
          <td>{m.symbol}</td>
          <td>{m.name}</td>
          <td class="num {m.change >= 0 ? 'up' : 'down'}">{pct(m.change)}</td>
          <td class="num">{money(m.turnover)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
