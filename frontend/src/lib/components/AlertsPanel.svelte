<script lang="ts">
  import type { AlertEventDto, AlertKind, AlertRuleInput, InstrumentDto } from "../types";

  // Панель алёртов: пользователь задаёт правила (цена/изменение), ядро
  // прогоняет их по сохранённым барам (`scan`) и возвращает сработавшие события.
  let {
    instruments = [],
    scan,
  }: {
    instruments: InstrumentDto[];
    scan: (rules: AlertRuleInput[]) => Promise<AlertEventDto[]>;
  } = $props();

  const KIND_LABELS: Record<AlertKind, string> = {
    priceAbove: "Цена выше",
    priceBelow: "Цена ниже",
    changeAbove: "Изм. выше, %",
    changeBelow: "Изм. ниже, %",
  };

  let rules = $state<AlertRuleInput[]>([]);
  let events = $state<AlertEventDto[]>([]);
  let scanError = $state<string | null>(null);

  // Поля формы.
  let symbol = $state("");
  let kind = $state<AlertKind>("priceAbove");
  let thresholdStr = $state("");

  // Значение по умолчанию для выпадающего списка инструментов.
  $effect(() => {
    if (symbol === "" && instruments.length > 0) symbol = instruments[0].symbol;
  });

  const isChangeKind = $derived(kind === "changeAbove" || kind === "changeBelow");

  async function refresh() {
    try {
      events = await scan(rules);
      scanError = null;
    } catch (e) {
      scanError = String(e);
    }
  }

  function addRule() {
    const raw = Number(thresholdStr);
    if (!symbol || Number.isNaN(raw)) return;
    // Для изменения порог вводится в процентах → переводим в доли.
    const threshold = isChangeKind ? raw / 100 : raw;
    rules = [...rules, { symbol, kind, threshold }];
    thresholdStr = "";
    void refresh();
  }

  function removeRule(i: number) {
    rules = rules.filter((_, idx) => idx !== i);
    void refresh();
  }

  function describe(r: AlertRuleInput): string {
    const t = r.kind === "changeAbove" || r.kind === "changeBelow"
      ? `${(r.threshold * 100).toFixed(2)}%`
      : r.threshold.toString();
    return `${KIND_LABELS[r.kind]} ${t}`;
  }

  function timeStr(ts: number): string {
    return new Date(ts * 1000).toLocaleString("ru-RU");
  }
</script>

<div class="alerts">
  <form class="rule-form" onsubmit={(e) => { e.preventDefault(); addRule(); }}>
    <select bind:value={symbol} aria-label="Инструмент">
      {#each instruments as inst (inst.symbol)}
        <option value={inst.symbol}>{inst.ticker}</option>
      {/each}
    </select>
    <select bind:value={kind} aria-label="Условие">
      {#each Object.entries(KIND_LABELS) as [k, label] (k)}
        <option value={k}>{label}</option>
      {/each}
    </select>
    <input
      type="number"
      step="any"
      placeholder={isChangeKind ? "%" : "порог"}
      bind:value={thresholdStr}
      aria-label="Порог"
    />
    <button type="submit">Добавить</button>
  </form>

  {#if rules.length > 0}
    <ul class="rules">
      {#each rules as r, i (r.symbol + r.kind + r.threshold + i)}
        <li>
          <span class="sym">{r.symbol.split("@")[0]}</span>
          <span class="cond">{describe(r)}</span>
          <button class="rm" onclick={() => removeRule(i)} aria-label="Удалить правило">×</button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if scanError}
    <div class="err">Ошибка: {scanError}</div>
  {/if}

  <div class="events-head">Срабатывания ({events.length})</div>
  {#if events.length === 0}
    <div class="empty">
      {rules.length === 0 ? "Добавьте правило выше" : "Срабатываний нет"}
    </div>
  {:else}
    <ul class="events">
      {#each events as ev (ev.symbol + ev.ts + ev.message)}
        <li>
          <span class="sym">{ev.symbol.split("@")[0]}</span>
          <span class="msg">{ev.message}</span>
          <span class="meta">{ev.price.toFixed(2)} · {timeStr(ev.ts)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .alerts {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 12px;
  }
  .rule-form {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  select,
  input,
  button {
    background: var(--bg-elev);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: 12px;
  }
  input {
    width: 80px;
  }
  button {
    cursor: pointer;
  }
  button:hover {
    border-color: var(--accent);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .rules li,
  .events li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 3px 6px;
    background: var(--bg-elev);
    border-radius: 4px;
  }
  .sym {
    font-weight: 600;
    color: var(--accent);
  }
  .cond,
  .msg {
    flex: 1;
  }
  .meta {
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .rm {
    padding: 0 6px;
    line-height: 1.4;
  }
  .events-head {
    color: var(--text-dim);
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.4px;
    border-top: 1px solid var(--border);
    padding-top: 6px;
  }
  .empty {
    color: var(--text-dim);
  }
  .err {
    color: var(--down);
  }
</style>
