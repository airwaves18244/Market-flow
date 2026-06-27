<script lang="ts">
  import type { TopMoverDto } from "../types";

  let { movers = [] }: { movers: TopMoverDto[] } = $props();

  function changeStr(change: number): string {
    const sign = change > 0 ? "+" : "";
    return `${sign}${(change * 100).toFixed(2)}%`;
  }
</script>

<table>
  <thead>
    <tr>
      <th>Тикер</th>
      <th>Название</th>
      <th>Сектор</th>
      <th class="num">Изм.</th>
      <th class="num">Цена</th>
    </tr>
  </thead>
  <tbody>
    {#each movers as mover (mover.symbol)}
      <tr>
        <td class="sym">{mover.ticker}</td>
        <td>{mover.name}</td>
        <td>{mover.sector ?? "Прочее"}</td>
        <td class="num" class:up={mover.change >= 0} class:down={mover.change < 0}>
          {changeStr(mover.change)}
        </td>
        <td class="num">{mover.lastClose.toFixed(2)}</td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  /* Таблица некликабельна — переопределяем глобальный курсор-указатель. */
  tbody tr {
    cursor: default;
  }
  .sym {
    font-weight: 600;
    color: var(--accent);
  }
</style>
