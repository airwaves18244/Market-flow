<script lang="ts">
  import type { OrderBookDto } from "../types";

  // Стакан (DOM): аски сверху (лучший — внизу у спреда), биды снизу.
  let { book = null }: { book: OrderBookDto | null } = $props();

  // Аски рисуем сверху вниз от худшего к лучшему, чтобы лучший аск примыкал
  // к спреду в центре.
  const asksDesc = $derived(book ? [...book.asks].reverse() : []);
  const bids = $derived(book ? book.bids : []);

  const maxSize = $derived(
    book
      ? Math.max(
          1,
          ...book.bids.map((l) => l.size),
          ...book.asks.map((l) => l.size),
        )
      : 1,
  );

  const spread = $derived(
    book && book.bids.length > 0 && book.asks.length > 0
      ? book.asks[0].price - book.bids[0].price
      : null,
  );

  function pct(size: number): number {
    return Math.round((size / maxSize) * 100);
  }
</script>

{#if !book || (bids.length === 0 && asksDesc.length === 0)}
  <div class="empty">Стакан пуст</div>
{:else}
  <div class="dom">
    <div class="side asks">
      {#each asksDesc as lvl (lvl.price)}
        <div class="row">
          <span class="bar ask" style="width:{pct(lvl.size)}%"></span>
          <span class="price down">{lvl.price.toFixed(2)}</span>
          <span class="size">{lvl.size}</span>
        </div>
      {/each}
    </div>

    <div class="spread">
      {#if spread != null}
        спред {spread.toFixed(2)}
      {:else}
        —
      {/if}
    </div>

    <div class="side bids">
      {#each bids as lvl (lvl.price)}
        <div class="row">
          <span class="bar bid" style="width:{pct(lvl.size)}%"></span>
          <span class="price up">{lvl.price.toFixed(2)}</span>
          <span class="size">{lvl.size}</span>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .dom {
    display: flex;
    flex-direction: column;
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .row {
    position: relative;
    display: flex;
    justify-content: space-between;
    padding: 2px 6px;
  }
  .bar {
    position: absolute;
    top: 0;
    bottom: 0;
    right: 0;
    z-index: 0;
    opacity: 0.18;
  }
  .bar.ask {
    background: var(--down);
  }
  .bar.bid {
    background: var(--up);
  }
  .price,
  .size {
    position: relative;
    z-index: 1;
  }
  .price.up {
    color: var(--up);
  }
  .price.down {
    color: var(--down);
  }
  .size {
    color: var(--text-dim);
  }
  .spread {
    text-align: center;
    font-size: 11px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.4px;
    padding: 4px 0;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    margin: 2px 0;
  }
  .empty {
    color: var(--text-dim);
    font-size: 12px;
    padding: 8px;
  }
</style>
