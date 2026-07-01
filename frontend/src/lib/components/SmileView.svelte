<script lang="ts">
  import { onMount } from "svelte";
  import Panel from "./Panel.svelte";
  import SmileChart from "./SmileChart.svelte";
  import { ipc } from "../ipc";
  import type { SmileFitDto, SmileModelDto, SmilePointInput } from "../types";

  // Улыбка волатильности: рыночные точки + калибровка одной из моделей.
  let forward = $state(100);
  let days = $state(30);
  let models = $state<SmileModelDto[]>([]);
  let model = $state("svi");
  let points = $state<SmilePointInput[]>([
    { strike: 80, iv: 0.34, weight: 1 },
    { strike: 90, iv: 0.3, weight: 2 },
    { strike: 100, iv: 0.27, weight: 3 },
    { strike: 110, iv: 0.29, weight: 2 },
    { strike: 120, iv: 0.33, weight: 1 },
  ]);
  let fit = $state<SmileFitDto | null>(null);
  let error = $state<string | null>(null);

  const t = $derived(Math.max(days, 0) / 365);

  async function calibrate() {
    error = null;
    try {
      fit = await ipc.smileFit({ model, points, forward, t });
    } catch (e) {
      error = String(e);
      fit = null;
    }
  }

  onMount(async () => {
    try {
      models = await ipc.listSmileModels();
    } catch {
      models = [{ id: "svi", name: "SVI" }];
    }
    await calibrate();
  });
</script>

<Panel title="Улыбка волатильности">
  <div class="smileview">
    <div class="side">
      <label>Форвард<input type="number" bind:value={forward} step="1" /></label>
      <label>Дней до эксп.<input type="number" bind:value={days} step="1" /></label>
      <label>
        Модель
        <select bind:value={model}>
          {#each models as m (m.id)}
            <option value={m.id}>{m.name}</option>
          {/each}
        </select>
      </label>
      <button onclick={calibrate}>Калибровать</button>

      {#if fit}
        <div class="params">
          <div class="rmse">RMSE: {(fit.rmse * 100).toFixed(3)}%</div>
          <table>
            <tbody>
              {#each fit.params as p (p.name)}
                <tr><th>{p.name}</th><td>{p.value.toFixed(4)}</td></tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
      {#if error}<div class="err">{error}</div>{/if}
    </div>

    <div class="chart"><SmileChart {points} {fit} /></div>
  </div>
</Panel>

<style>
  .smileview {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr);
    gap: 12px;
  }
  .side {
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
    background: var(--accent);
    border: none;
    border-radius: 4px;
    color: #fff;
    padding: 6px;
    font-size: 12px;
    cursor: pointer;
  }
  .params {
    font-size: 12px;
  }
  .rmse {
    color: var(--text-dim);
    margin-bottom: 4px;
  }
  .params table {
    width: 100%;
    border-collapse: collapse;
  }
  .params th {
    text-align: left;
    color: var(--text-dim);
    font-weight: 500;
  }
  .params td {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .chart {
    min-height: 280px;
  }
  .err {
    color: #f5646c;
    font-size: 12px;
  }
</style>
