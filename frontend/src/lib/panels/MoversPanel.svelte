<script lang="ts">
  import type { Mover } from "../types";

  let { movers }: { movers: Mover[] } = $props();

  const pct = (x: number) => `${(x * 100).toFixed(2)}%`;
  const money = (x: number) =>
    new Intl.NumberFormat("ru-RU", { notation: "compact", maximumFractionDigits: 1 }).format(x);
</script>

<section class="panel">
  <h2>Топ-движения</h2>
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
</section>
