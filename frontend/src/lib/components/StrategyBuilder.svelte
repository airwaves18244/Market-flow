<script lang="ts">
  import Panel from "./Panel.svelte";
  import PayoffChart from "./PayoffChart.svelte";
  import { ipc } from "../ipc";
  import { fmtFixed } from "../format";
  import type { LegKind, LegSide, StrategyEvalDto, StrategyLegInput } from "../types";

  // Конструктор стратегий: ноги + шаблоны + диаграмма payoff + греки/безубытки.
  let forward = $state(100);
  let volPct = $state(30);
  let legs = $state<StrategyLegInput[]>([
    { kind: "call", side: "long", strike: 100, expiryT: 0.08, quantity: 1, entryPrice: 5 },
  ]);
  let result = $state<StrategyEvalDto | null>(null);
  let error = $state<string | null>(null);

  type Template = { id: string; label: string; build: () => StrategyLegInput[] };
  const T = (kind: LegKind, side: LegSide, strike: number, entryPrice: number): StrategyLegInput => ({
    kind,
    side,
    strike,
    expiryT: 0.08,
    quantity: 1,
    entryPrice,
  });
  const templates: Template[] = [
    { id: "vertical", label: "Вертикаль", build: () => [T("call", "long", 95, 7), T("call", "short", 110, 2)] },
    { id: "straddle", label: "Стрэддл", build: () => [T("call", "long", 100, 5), T("put", "long", 100, 5)] },
    { id: "strangle", label: "Стрэнгл", build: () => [T("call", "long", 110, 3), T("put", "long", 90, 3)] },
    {
      id: "butterfly",
      label: "Бабочка",
      build: () => [
        T("call", "long", 90, 12),
        { ...T("call", "short", 100, 5), quantity: 2 },
        T("call", "long", 110, 2),
      ],
    },
    {
      id: "condor",
      label: "Кондор",
      build: () => [
        T("call", "long", 85, 16),
        T("call", "short", 95, 8),
        T("call", "short", 110, 3),
        T("call", "long", 120, 1),
      ],
    },
    {
      id: "covered",
      label: "Покрытый колл",
      build: () => [T("underlying", "long", 100, 100), T("call", "short", 110, 3)],
    },
  ];

  let activePreset = $state("");
  function applyTemplate(id: string) {
    const t = templates.find((x) => x.id === id);
    if (t) {
      legs = t.build();
      activePreset = id;
    }
  }

  function addLeg() {
    legs = [...legs, T("call", "long", forward, 1)];
  }
  function removeLeg(i: number) {
    legs = legs.filter((_, idx) => idx !== i);
  }

  async function evaluate() {
    error = null;
    try {
      const strikes = legs.map((l) => l.strike);
      const lo = Math.min(forward * 0.7, ...strikes) * 0.95;
      const hi = Math.max(forward * 1.3, ...strikes) * 1.05;
      result = await ipc.strategyEval({
        legs,
        priceLo: lo,
        priceHi: hi,
        forward,
        vol: volPct / 100,
      });
    } catch (e) {
      error = String(e);
      result = null;
    }
  }

</script>

<Panel title="Конструктор стратегий">
  <div class="builder">
    <div class="controls">
      <label>Форвард<input type="number" bind:value={forward} step="1" /></label>
      <label>Волатильность, %<input type="number" bind:value={volPct} step="1" /></label>
      <div class="presets">
        <span class="field-label">Шаблон</span>
        <div class="preset-chips">
          {#each templates as t (t.id)}
            <button
              class="chip-btn"
              class:active={activePreset === t.id}
              onclick={() => applyTemplate(t.id)}>{t.label}</button
            >
          {/each}
        </div>
      </div>
    </div>

    <table class="legs">
      <thead>
        <tr><th>Тип</th><th>Сторона</th><th>Страйк</th><th>Кол-во</th><th>Премия</th><th></th></tr>
      </thead>
      <tbody>
        {#each legs as leg, i (i)}
          <tr>
            <td>
              <select bind:value={leg.kind}>
                <option value="call">call</option>
                <option value="put">put</option>
                <option value="underlying">базовый</option>
              </select>
            </td>
            <td>
              <select bind:value={leg.side}>
                <option value="long">long</option>
                <option value="short">short</option>
              </select>
            </td>
            <td><input type="number" bind:value={leg.strike} step="1" /></td>
            <td><input type="number" bind:value={leg.quantity} step="1" /></td>
            <td><input type="number" bind:value={leg.entryPrice} step="0.1" /></td>
            <td><button class="rm" onclick={() => removeLeg(i)} aria-label="удалить">×</button></td>
          </tr>
        {/each}
      </tbody>
    </table>

    <div class="actions">
      <button onclick={addLeg}>+ нога</button>
      <button class="primary" onclick={evaluate}>Оценить</button>
    </div>

    {#if error}
      <div class="err">{error}</div>
    {:else if result}
      <div class="chart"><PayoffChart payoff={result.payoff} breakevens={result.breakevens} /></div>
      <div class="summary">
        <span>Безубыток: {result.breakevens.map((b) => b.toFixed(1)).join(", ") || "—"}</span>
        <span>Макс. прибыль: {fmtFixed(result.maxProfit)}</span>
        <span>Макс. убыток: {fmtFixed(result.maxLoss)}</span>
        <span>Дебет: {fmtFixed(result.netCost)}</span>
      </div>
      <table class="greeks">
        <tbody>
          <tr>
            <th>Δ</th><td>{fmtFixed(result.greeks.delta, 3)}</td>
            <th>Γ</th><td>{fmtFixed(result.greeks.gamma, 5)}</td>
            <th>Vega</th><td>{fmtFixed(result.greeks.vega, 3)}</td>
            <th>Θ</th><td>{fmtFixed(result.greeks.theta, 3)}</td>
          </tr>
        </tbody>
      </table>
    {:else}
      <div class="empty">Соберите ноги и нажмите «Оценить».</div>
    {/if}
  </div>
</Panel>

<style>
  .builder {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .controls {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    align-items: flex-end;
  }
  .presets .field-label {
    margin-bottom: 3px;
  }
  .preset-chips {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    flex-direction: column;
    font-size: 11px;
    color: var(--text-dim);
    gap: 2px;
  }
  input,
  select {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    padding: 4px 6px;
    font-size: 12px;
  }
  .legs {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  .legs th {
    text-align: left;
    color: var(--text-dim);
    font-weight: 500;
    padding: 2px 4px;
  }
  .legs td {
    padding: 2px 4px;
  }
  .legs input {
    width: 68px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  button {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    padding: 5px 10px;
    font-size: 12px;
    cursor: pointer;
  }
  button.primary {
    background: var(--accent);
    border: none;
    color: #fff;
  }
  button.rm {
    padding: 2px 8px;
    color: #f5646c;
  }
  .chart {
    height: 280px;
  }
  .summary {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--text-dim);
  }
  .greeks {
    font-size: 12px;
    border-collapse: collapse;
  }
  .greeks th {
    color: var(--text-dim);
    font-weight: 500;
    padding: 2px 6px 2px 0;
  }
  .greeks td {
    padding: 2px 14px 2px 0;
    font-variant-numeric: tabular-nums;
  }
  .empty,
  .err {
    font-size: 12px;
    color: var(--text-dim);
  }
  .err {
    color: #f5646c;
  }
</style>
