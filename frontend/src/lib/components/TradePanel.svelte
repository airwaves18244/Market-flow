<script lang="ts">
  import { onMount } from "svelte";
  import Panel from "./Panel.svelte";
  import InstrumentList from "./InstrumentList.svelte";
  import { ipc, onFill } from "../ipc";
  import type {
    AccountDto,
    InstrumentDto,
    OrderBookDto,
    OrderDto,
    OrderInput,
    OrderKind,
    OrderSide,
    PositionDto,
    Tif,
  } from "../types";

  let {
    instruments,
    selected,
    onSelect,
  }: {
    instruments: InstrumentDto[];
    selected: string;
    onSelect: (symbol: string) => void;
  } = $props();

  let book = $state<OrderBookDto | null>(null);
  let orders = $state<OrderDto[]>([]);
  let positions = $state<PositionDto[]>([]);
  let account = $state<AccountDto | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);

  // Тикет заявки.
  let side = $state<OrderSide>("buy");
  let qty = $state(1);
  let kind = $state<OrderKind>("market");
  let price = $state<number | null>(null);
  let tif = $state<Tif>("gtc");

  async function refresh() {
    [orders, positions, account] = await Promise.all([
      ipc.orderBlotter(),
      ipc.positions(),
      ipc.account(),
    ]);
  }

  async function loadBook() {
    try {
      book = await ipc.orderBook(selected, 10);
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    void selected;
    loadBook();
  });

  // Клик по уровню стакана подставляет цену (и сторону) в тикет.
  function pickPrice(p: number, asksSide: boolean) {
    price = p;
    if (kind === "market") kind = "limit";
    side = asksSide ? "buy" : "sell";
  }

  async function submit() {
    error = null;
    notice = null;
    const order: OrderInput = {
      symbol: selected,
      side,
      qty,
      kind,
      price: kind === "market" ? null : price,
      tif,
    };
    try {
      const res = await ipc.submitOrder(order);
      notice = `Заявка #${res.order.id}: ${res.order.status}${res.fills.length ? `, исполнений: ${res.fills.length}` : ""}`;
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function cancel(id: number) {
    try {
      await ipc.cancelOrder(id);
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
    try {
      await refresh();
    } catch (e) {
      error = String(e);
    }
    // Живые исполнения симулятора обновляют блоттер/позиции.
    await onFill(() => {
      refresh().catch((e) => (error = String(e)));
    });
  });

  const fmt = (n: number) => n.toLocaleString("ru-RU", { maximumFractionDigits: 2 });
  const maxSize = $derived(
    Math.max(1, ...(book?.bids ?? []).map((l) => l.size), ...(book?.asks ?? []).map((l) => l.size)),
  );
</script>

<main class="grid">
  <Panel title={`DOM — ${selected}`}>
    {#if book}
      <div class="ladder">
        {#each [...book.asks].reverse() as lvl (lvl.price)}
          <button class="row ask" onclick={() => pickPrice(lvl.price, true)}>
            <span class="depth ask-depth" style="width:{(lvl.size / maxSize) * 100}%"></span>
            <span class="px">{fmt(lvl.price)}</span>
            <span class="sz num">{fmt(lvl.size)}</span>
          </button>
        {/each}
        <div class="spread">
          спред {book.asks[0] && book.bids[0] ? fmt(book.asks[0].price - book.bids[0].price) : "—"}
        </div>
        {#each book.bids as lvl (lvl.price)}
          <button class="row bid" onclick={() => pickPrice(lvl.price, false)}>
            <span class="depth bid-depth" style="width:{(lvl.size / maxSize) * 100}%"></span>
            <span class="px">{fmt(lvl.price)}</span>
            <span class="sz num">{fmt(lvl.size)}</span>
          </button>
        {/each}
      </div>
    {:else}
      <p class="empty">Нет стакана для {selected}.</p>
    {/if}
  </Panel>

  <Panel title="Тикет заявки">
    <div class="form">
      <div class="sides">
        <button class:active={side === "buy"} class="buy" onclick={() => (side = "buy")}>Покупка</button>
        <button class:active={side === "sell"} class="sell" onclick={() => (side = "sell")}>Продажа</button>
      </div>
      <label>Объём<input type="number" bind:value={qty} min="0" step="any" /></label>
      <label>
        Тип
        <select bind:value={kind}>
          <option value="market">Рыночная</option>
          <option value="limit">Лимитная</option>
          <option value="stop">Стоп</option>
        </select>
      </label>
      {#if kind !== "market"}
        <label>Цена<input type="number" bind:value={price} step="any" /></label>
      {/if}
      <label>
        TIF
        <select bind:value={tif}>
          <option value="gtc">GTC</option>
          <option value="day">Day</option>
          <option value="ioc">IOC</option>
        </select>
      </label>
      <button class="send" class:buy={side === "buy"} class:sell={side === "sell"} onclick={submit}>
        Отправить {side === "buy" ? "покупку" : "продажу"}
      </button>
      {#if notice}<div class="notice">{notice}</div>{/if}
      {#if error}<div class="error">{error}</div>{/if}
    </div>
  </Panel>

  <Panel title="Счёт">
    {#if account}
      <div class="account">
        <div><span>Наличность</span><b class="num">{fmt(account.cash)}</b></div>
        <div>
          <span>Реализ. P&L</span>
          <b class="num" class:up={account.realizedPnl > 0} class:down={account.realizedPnl < 0}>{fmt(account.realizedPnl)}</b>
        </div>
      </div>
    {/if}
  </Panel>

  <Panel title="Инструменты">
    <InstrumentList items={instruments} {selected} onSelect={onSelect} />
  </Panel>

  <Panel title={`Позиции (${positions.length})`}>
    <table>
      <thead>
        <tr><th>Инструмент</th><th class="num">Кол-во</th><th class="num">Ср. цена</th></tr>
      </thead>
      <tbody>
        {#each positions as p (p.symbol)}
          <tr>
            <td>{p.symbol}</td>
            <td class="num" class:up={p.qty > 0} class:down={p.qty < 0}>{fmt(p.qty)}</td>
            <td class="num">{fmt(p.avgPrice)}</td>
          </tr>
        {/each}
        {#if positions.length === 0}
          <tr><td colspan="3" class="empty">Нет открытых позиций.</td></tr>
        {/if}
      </tbody>
    </table>
  </Panel>

  <Panel title={`Заявки (${orders.length})`}>
    <table>
      <thead>
        <tr><th>#</th><th>Сторона</th><th>Тип</th><th class="num">Кол-во</th><th class="num">Цена</th><th>Статус</th><th></th></tr>
      </thead>
      <tbody>
        {#each orders as o (o.id)}
          <tr>
            <td>{o.id}</td>
            <td class:up={o.side === "buy"} class:down={o.side === "sell"}>{o.side === "buy" ? "Покупка" : "Продажа"}</td>
            <td>{o.kind}</td>
            <td class="num">{fmt(o.qty)}</td>
            <td class="num">{o.price !== null ? fmt(o.price) : "—"}</td>
            <td>{o.status}</td>
            <td><button class="cancel" onclick={() => cancel(o.id)}>✕</button></td>
          </tr>
        {/each}
        {#if orders.length === 0}
          <tr><td colspan="7" class="empty">Активных заявок нет.</td></tr>
        {/if}
      </tbody>
    </table>
  </Panel>
</main>

<style>
  .ladder {
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-variant-numeric: tabular-nums;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border: none;
    background: var(--bg-elev);
    color: var(--text);
    padding: 3px 8px;
    font-size: 13px;
    cursor: pointer;
    overflow: hidden;
  }
  .row:hover {
    outline: 1px solid var(--accent);
  }
  .depth {
    position: absolute;
    top: 0;
    bottom: 0;
    right: 0;
    z-index: 0;
  }
  .ask-depth {
    background: rgba(239, 83, 80, 0.16);
  }
  .bid-depth {
    background: rgba(38, 166, 154, 0.16);
  }
  .px,
  .sz {
    position: relative;
    z-index: 1;
  }
  .ask .px {
    color: var(--down);
  }
  .bid .px {
    color: var(--up);
  }
  .spread {
    text-align: center;
    font-size: 11px;
    color: var(--text-dim);
    padding: 2px;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .form input,
  .form select {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text);
    padding: 5px 8px;
    font-size: 13px;
  }
  .sides {
    display: flex;
    gap: 6px;
  }
  .sides button {
    flex: 1;
    padding: 6px;
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--text-dim);
    border-radius: 6px;
    cursor: pointer;
  }
  .sides .buy.active {
    background: rgba(38, 166, 154, 0.2);
    color: var(--up);
    border-color: var(--up);
  }
  .sides .sell.active {
    background: rgba(239, 83, 80, 0.2);
    color: var(--down);
    border-color: var(--down);
  }
  .send {
    margin-top: 4px;
    border: none;
    border-radius: 6px;
    padding: 9px;
    font-weight: 600;
    cursor: pointer;
    color: #06101f;
  }
  .send.buy {
    background: var(--up);
  }
  .send.sell {
    background: var(--down);
  }
  .notice {
    font-size: 12px;
    color: var(--accent);
  }
  .cancel {
    border: none;
    background: transparent;
    color: var(--down);
    cursor: pointer;
    font-size: 13px;
  }
  .account {
    display: flex;
    gap: 16px;
  }
  .account div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .account span {
    font-size: 11px;
    color: var(--text-dim);
  }
  .account b {
    font-size: 16px;
  }
  .empty {
    color: var(--text-dim);
    font-size: 13px;
    padding: 8px;
    text-align: center;
  }
</style>
