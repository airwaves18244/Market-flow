<script lang="ts">
  import type { OrderBookDto } from "../types";

  let { book = null }: { book: OrderBookDto | null } = $props();

  // Максимум кумулятива по обеим сторонам — для ширины полос глубины.
  let maxCum = $derived.by(() => {
    if (!book) return 1;
    const b = book.bids.at(-1)?.cumulative ?? 0;
    const a = book.asks.at(-1)?.cumulative ?? 0;
    return Math.max(b, a, 1);
  });

  function fmt(n: number): string {
    return n.toLocaleString("ru-RU", { maximumFractionDigits: 2 });
  }
</script>

{#if book}
  <div class="dom">
    <div class="head">
      <span>Спред: {book.spread != null ? fmt(book.spread) : "—"}</span>
      <span>Mid: {book.mid != null ? fmt(book.mid) : "—"}</span>
      <span class:pos={(book.imbalance ?? 0) >= 0} class:neg={(book.imbalance ?? 0) < 0}>
        Дисбаланс: {book.imbalance != null ? (book.imbalance * 100).toFixed(0) + "%" : "—"}
      </span>
    </div>

    <!-- asks: от худшей вниз к лучшей, чтобы лучшая прилегала к спреду -->
    <div class="side asks">
      {#each [...book.asks].reverse() as lvl (lvl.price)}
        <div class="row">
          <span class="depth ask" style="width: {(lvl.cumulative / maxCum) * 100}%"></span>
          <span class="size">{fmt(lvl.size)}</span>
          <span class="price ask-px">{fmt(lvl.price)}</span>
        </div>
      {/each}
    </div>

    <div class="mid-line">
      {book.mid != null ? fmt(book.mid) : ""}
    </div>

    <div class="side bids">
      {#each book.bids as lvl (lvl.price)}
        <div class="row">
          <span class="depth bid" style="width: {(lvl.cumulative / maxCum) * 100}%"></span>
          <span class="size">{fmt(lvl.size)}</span>
          <span class="price bid-px">{fmt(lvl.price)}</span>
        </div>
      {/each}
    </div>
  </div>
{:else}
  <div class="empty">Нет данных стакана</div>
{/if}

<style>
  .dom {
    display: flex;
    flex-direction: column;
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 2px 4px 6px;
    color: var(--text-dim);
    font-size: 11px;
  }
  .head .pos {
    color: var(--up, #3fb950);
  }
  .head .neg {
    color: var(--down, #f85149);
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    padding: 1px 6px;
    height: 18px;
  }
  .depth {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    z-index: 0;
    opacity: 0.18;
  }
  .depth.ask {
    background: var(--down, #f85149);
  }
  .depth.bid {
    background: var(--up, #3fb950);
  }
  .size,
  .price {
    position: relative;
    z-index: 1;
  }
  .size {
    color: var(--text-dim);
    min-width: 54px;
    text-align: right;
  }
  .price {
    min-width: 64px;
    text-align: right;
    font-weight: 600;
  }
  .ask-px {
    color: var(--down, #f85149);
  }
  .bid-px {
    color: var(--up, #3fb950);
  }
  .mid-line {
    padding: 3px 6px;
    text-align: right;
    color: var(--text-dim);
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
  }
  .empty {
    padding: 16px;
    color: var(--text-dim);
    text-align: center;
  }
</style>
