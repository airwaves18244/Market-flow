<script lang="ts">
  import Panel from "./Panel.svelte";
  import { ipc } from "../ipc";
  import { fmtFixed } from "../format";
  import type { OptionKind, OptionPriceDto, PricingModel } from "../types";

  // Калькулятор: цена/греки/IV опциона по модели Блэк-76/Башелье.
  let forward = $state(100);
  let strike = $state(100);
  let days = $state(30);
  let volPct = $state(30);
  let ratePct = $state(0);
  let kind = $state<OptionKind>("call");
  let model = $state<PricingModel>("black76");
  let marketPrice = $state<number | null>(null);

  let result = $state<OptionPriceDto | null>(null);
  let iv = $state<number | null>(null);
  let error = $state<string | null>(null);

  const t = $derived(Math.max(days, 0) / 365);

  async function calc() {
    error = null;
    try {
      result = await ipc.optionPrice({
        forward,
        strike,
        t,
        vol: volPct / 100,
        rate: ratePct / 100,
        kind,
        model,
      });
      if (marketPrice != null && marketPrice > 0) {
        const r = await ipc.optionImpliedVol({
          marketPrice,
          forward,
          strike,
          t,
          rate: ratePct / 100,
          kind,
          model,
        });
        iv = r.iv;
      } else {
        iv = null;
      }
    } catch (e) {
      error = String(e);
      result = null;
    }
  }

</script>

<Panel title="Калькулятор">
  <div class="calc">
    <div class="form">
      <label>Форвард<input type="number" bind:value={forward} step="1" /></label>
      <label>Страйк<input type="number" bind:value={strike} step="1" /></label>
      <label>Дней до эксп.<input type="number" bind:value={days} step="1" /></label>
      <label>Волатильность, %<input type="number" bind:value={volPct} step="1" /></label>
      <label>Ставка r, %<input type="number" bind:value={ratePct} step="0.5" /></label>
      <label>
        Тип
        <select bind:value={kind}>
          <option value="call">Call</option>
          <option value="put">Put</option>
        </select>
      </label>
      <label>
        Модель
        <select bind:value={model}>
          <option value="black76">Black-76</option>
          <option value="bachelier">Bachelier</option>
        </select>
      </label>
      <label>Рыночная цена (для IV)<input type="number" bind:value={marketPrice} step="0.1" /></label>
      <button onclick={calc}>Рассчитать</button>
    </div>

    {#if error}
      <div class="err">{error}</div>
    {:else if result}
      <table class="res">
        <tbody>
          <tr><th>Цена</th><td>{fmtFixed(result.price, 4)}</td></tr>
          <tr><th>Delta</th><td>{fmtFixed(result.greeks.delta, 4)}</td></tr>
          <tr><th>Gamma</th><td>{fmtFixed(result.greeks.gamma, 6)}</td></tr>
          <tr><th>Vega</th><td>{fmtFixed(result.greeks.vega, 4)}</td></tr>
          <tr><th>Theta</th><td>{fmtFixed(result.greeks.theta, 4)}</td></tr>
          <tr><th>Rho</th><td>{fmtFixed(result.greeks.rho, 4)}</td></tr>
          {#if iv != null}
            <tr><th>IV (из цены)</th><td>{(iv * 100).toFixed(2)}%</td></tr>
          {/if}
        </tbody>
      </table>
    {:else}
      <div class="empty">Задайте параметры и нажмите «Рассчитать».</div>
    {/if}
  </div>
</Panel>

<style>
  .calc {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 12px;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 6px;
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
  button {
    margin-top: 4px;
    background: var(--accent);
    border: none;
    border-radius: 4px;
    color: #fff;
    padding: 6px;
    font-size: 12px;
    cursor: pointer;
  }
  .res {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    align-self: start;
  }
  .res th {
    text-align: left;
    color: var(--text-dim);
    font-weight: 500;
    padding: 3px 8px 3px 0;
  }
  .res td {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .empty,
  .err {
    font-size: 12px;
    color: var(--text-dim);
    align-self: center;
  }
  .err {
    color: #f5646c;
  }
</style>
