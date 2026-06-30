<script lang="ts">
  // Вкладка «Торговля» — ПРОТОТИП. Заявки симулируются и НЕ отправляются на
  // биржу: текущая сборка READ-ONLY. Реальный путь (AccountsService/OrdersService
  // + подтверждения/kill-switch) описан в ROADMAP.md.

  let { symbol = "SBER", price = 308.5 }: { symbol?: string; price?: number } = $props();

  let side = $state<"buy" | "sell">("buy");
  let type = $state<"market" | "limit" | "stop">("limit");
  let qty = $state(10);
  let message = $state<string | null>(null);

  const TYPES: Array<{ k: "market" | "limit" | "stop"; label: string }> = [
    { k: "market", label: "Рыночная" },
    { k: "limit", label: "Лимит" },
    { k: "stop", label: "Стоп" },
  ];

  function submit() {
    message = "Заявка отклонена · прототип READ-ONLY (бэкенд не подключён).";
  }

  const est = $derived(price * qty);

  type Pos = { sym: string; qty: number; avg: number; last: number };
  const positions: Pos[] = [
    { sym: "SBER", qty: 120, avg: 298.4, last: 308.5 },
    { sym: "YDEX", qty: 30, avg: 4510, last: 4380 },
    { sym: "LKOH", qty: 8, avg: 6980, last: 7120 },
    { sym: "GMKN", qty: 200, avg: 152.0, last: 158.2 },
    { sym: "GAZP", qty: 500, avg: 131.2, last: 128.74 },
  ];

  const orders = [
    { t: "18:11:50", sym: "SBER", buy: true, type: "Лимит", qty: 50, price: 308.4 },
    { t: "17:40:20", sym: "LKOH", buy: true, type: "Стоп", qty: 5, price: 7100 },
    { t: "17:22:11", sym: "GMKN", buy: false, type: "Лимит", qty: 100, price: 159.0 },
  ];

  const fmt = (n: number) =>
    n.toLocaleString("ru-RU", { maximumFractionDigits: 2 });

  const totPnl = $derived(
    positions.reduce((s, p) => s + (p.last - p.avg) * p.qty, 0),
  );
  const totVal = $derived(positions.reduce((s, p) => s + p.last * p.qty, 0));

  const account = $derived([
    { k: "Стоимость портфеля", v: `₽${fmt(totVal)}` },
    { k: "Свободные средства", v: "₽420 000" },
    { k: "PnL дня", v: `${totPnl >= 0 ? "+" : ""}₽${fmt(totPnl)}` },
    { k: "Исп. маржа", v: "₽280 000" },
    { k: "Покуп. способность", v: "₽840 000" },
  ]);
</script>

<div class="trade">
  <div class="banner">
    <span class="dot"></span>
    Прототип торгового терминала · заявки симулируются и НЕ отправляются на биржу
    (текущая сборка — READ-ONLY).
  </div>

  <div class="cols">
    <div class="card ticket">
      <div class="card-head"><span>Заявка</span><span class="mono">{symbol}@MISX</span></div>
      <div class="ticket-body">
        <div class="seg side-seg">
          <button class:buy-on={side === "buy"} onclick={() => (side = "buy")}>Купить</button>
          <button class:sell-on={side === "sell"} onclick={() => (side = "sell")}>Продать</button>
        </div>

        <div class="field">
          <span class="cap">Тип</span>
          <div class="seg">
            {#each TYPES as t (t.k)}
              <button class="pill" class:active={type === t.k} onclick={() => (type = t.k)}>
                {t.label}
              </button>
            {/each}
          </div>
        </div>

        <div class="field">
          <span class="cap">Количество, лотов</span>
          <div class="stepper">
            <button onclick={() => (qty = Math.max(1, qty - 1))}>−</button>
            <span class="qty">{qty}</span>
            <button onclick={() => (qty += 1)}>+</button>
          </div>
        </div>

        <div class="field">
          <span class="cap">Цена</span>
          <div class="price-box"><b>{fmt(price)}</b><span>₽ · посл.</span></div>
        </div>

        <div class="est"><span>Оценочно</span><b>₽{fmt(est)}</b></div>

        <button class="submit" class:buy={side === "buy"} class:sell={side === "sell"} onclick={submit}>
          {side === "buy" ? "Купить" : "Продать"} {symbol}
        </button>

        {#if message}
          <div class="reject">{message}</div>
        {/if}
      </div>
    </div>

    <div class="mid">
      <div class="card">
        <div class="card-head">
          <span>Позиции</span>
          <span class="mono" class:up={totPnl >= 0} class:down={totPnl < 0}>
            {totPnl >= 0 ? "+" : ""}₽{fmt(totPnl)}
          </span>
        </div>
        <table>
          <thead>
            <tr><th>Тикер</th><th class="num">Кол-во</th><th class="num">Средн.</th><th class="num">Цена</th><th class="num">PnL</th><th class="num">%</th></tr>
          </thead>
          <tbody>
            {#each positions as p (p.sym)}
              {@const pnl = (p.last - p.avg) * p.qty}
              {@const pct = (p.last / p.avg - 1) * 100}
              <tr>
                <td>{p.sym}</td>
                <td class="num">{p.qty}</td>
                <td class="num">{fmt(p.avg)}</td>
                <td class="num">{fmt(p.last)}</td>
                <td class="num" class:up={pnl >= 0} class:down={pnl < 0}>{pnl >= 0 ? "+" : ""}{fmt(pnl)}</td>
                <td class="num" class:up={pct >= 0} class:down={pct < 0}>{pct >= 0 ? "+" : ""}{pct.toFixed(2)}%</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <div class="card">
        <div class="card-head"><span>Активные заявки</span></div>
        <table>
          <thead>
            <tr><th>Время</th><th>Тикер</th><th>Сторона</th><th>Тип</th><th class="num">Кол-во</th><th class="num">Цена</th></tr>
          </thead>
          <tbody>
            {#each orders as o (o.t)}
              <tr>
                <td class="dim">{o.t}</td>
                <td>{o.sym}</td>
                <td class:up={o.buy} class:down={!o.buy}>{o.buy ? "Купля" : "Продажа"}</td>
                <td class="dim">{o.type}</td>
                <td class="num">{o.qty}</td>
                <td class="num">{fmt(o.price)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>

    <div class="card">
      <div class="card-head"><span>Счёт</span></div>
      <div class="account">
        {#each account as a (a.k)}
          <div class="acct-row"><span>{a.k}</span><b>{a.v}</b></div>
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .trade {
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 12px;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #e0a23a;
    background: rgba(224, 162, 58, 0.08);
    border: 1px solid rgba(224, 162, 58, 0.25);
    border-radius: 6px;
    padding: 8px 12px;
  }
  .banner .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #e0a23a;
    flex: none;
  }
  .cols {
    display: grid;
    grid-template-columns: 288px 1fr 250px;
    gap: 10px;
    align-items: start;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-panel);
    overflow: hidden;
  }
  .card-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    font-weight: 600;
  }
  .mono {
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
  }
  .ticket-body {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .seg {
    display: flex;
    gap: 2px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px;
  }
  .seg button {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text-dim);
    padding: 7px 0;
    cursor: pointer;
    font-weight: 600;
    border-radius: 4px;
  }
  .side-seg .buy-on {
    background: rgba(38, 166, 154, 0.22);
    color: var(--text);
  }
  .side-seg .sell-on {
    background: rgba(239, 83, 80, 0.22);
    color: var(--text);
  }
  .pill.active {
    background: rgba(79, 156, 249, 0.2);
    color: var(--text);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .cap {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
  }
  .stepper {
    display: flex;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .stepper button {
    border: none;
    background: var(--bg);
    color: var(--text);
    font-size: 15px;
    padding: 7px 14px;
    cursor: pointer;
  }
  .qty {
    flex: 1;
    text-align: center;
    font-weight: 600;
    font-size: 14px;
    font-variant-numeric: tabular-nums;
  }
  .price-box {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 11px;
  }
  .price-box b {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .price-box span {
    font-size: 10px;
    color: var(--text-dim);
  }
  .est {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--text-dim);
  }
  .est b {
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  .submit {
    border: none;
    border-radius: 7px;
    padding: 11px;
    font-weight: 600;
    cursor: pointer;
    color: #0b0d12;
  }
  .submit.buy {
    background: var(--up);
  }
  .submit.sell {
    background: var(--down);
  }
  .reject {
    font-size: 11px;
    color: var(--down);
    background: rgba(239, 83, 80, 0.1);
    border: 1px solid rgba(239, 83, 80, 0.3);
    border-radius: 6px;
    padding: 8px 10px;
    line-height: 1.35;
  }
  .mid {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-variant-numeric: tabular-nums;
  }
  .account {
    padding: 4px 13px;
  }
  .acct-row {
    display: flex;
    justify-content: space-between;
    padding: 9px 0;
    border-bottom: 1px solid var(--border);
  }
  .acct-row span {
    color: var(--text-dim);
  }
  .acct-row b {
    font-variant-numeric: tabular-nums;
  }
  .up {
    color: var(--up);
  }
  .down {
    color: var(--down);
  }
  .dim {
    color: var(--text-dim);
  }
  @media (max-width: 900px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
