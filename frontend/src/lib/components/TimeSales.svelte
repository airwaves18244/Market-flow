<script lang="ts">
  import type { TradeDto } from "../types";

  // Лента обезличенных сделок (Time&Sales). Самые свежие — сверху.
  let { trades = [] }: { trades: TradeDto[] } = $props();

  function timeStr(ts: number): string {
    const d = new Date(ts * 1000);
    return d.toLocaleTimeString("ru-RU", { hour12: false });
  }
</script>

{#if trades.length === 0}
  <div class="empty">Нет сделок</div>
{:else}
  <table>
    <thead>
      <tr>
        <th>Время</th>
        <th class="num">Цена</th>
        <th class="num">Объём</th>
      </tr>
    </thead>
    <tbody>
      {#each trades as t (t.ts + "-" + t.price + "-" + t.size)}
        <tr>
          <td class="dim">{timeStr(t.ts)}</td>
          <td
            class="num"
            class:up={t.buyerInitiated === true}
            class:down={t.buyerInitiated === false}
          >
            {t.price.toFixed(2)}
          </td>
          <td class="num">{t.size}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  tbody tr {
    cursor: default;
  }
  .dim {
    color: var(--text-dim);
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .up {
    color: var(--up);
  }
  .down {
    color: var(--down);
  }
  .empty {
    color: var(--text-dim);
    font-size: 12px;
    padding: 8px;
  }
</style>
