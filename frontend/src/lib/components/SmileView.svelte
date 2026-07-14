<script lang="ts">
  import { onMount } from "svelte";
  import SmileChart, { type SmileCurveLine } from "./SmileChart.svelte";
  import { loadSettings } from "../settings";
  import { ipc } from "../ipc";
  import type { SmileFitDto, SmileModelDto, SmilePointInput } from "../types";

  // Улыбка волатильности (Фаза 12): рыночные точки + калибровка всех доступных
  // моделей одновременно. Карточки показывают параметры и RMSE каждой модели;
  // чипы включают/выключают кривые, «активная» модель задаёт рабочую улыбку.
  const settings = loadSettings();

  let forward = $state(300);
  let days = $state(30);
  let models = $state<SmileModelDto[]>([]);
  let visible = $state<Record<string, boolean>>({});
  let activeModel = $state<string>(settings.defaultSmile);
  let fits = $state<Record<string, SmileFitDto>>({});
  let error = $state<string | null>(null);

  // Демо-доска: рыночные IV-точки (страйк/IV/OI-вес). В боевом режиме приходят
  // из опционной доски ALGOPACK.
  const points: SmilePointInput[] = [
    { strike: 260, iv: 0.42, weight: 0.4 },
    { strike: 275, iv: 0.36, weight: 0.7 },
    { strike: 290, iv: 0.32, weight: 1 },
    { strike: 300, iv: 0.3, weight: 1.2 },
    { strike: 310, iv: 0.31, weight: 1 },
    { strike: 325, iv: 0.35, weight: 0.6 },
    { strike: 340, iv: 0.4, weight: 0.35 },
  ];

  const PALETTE = ["#4f9cf9", "#26a69a", "#f5a623", "#a371f7", "#ef5350"];
  const colorOf = (id: string) =>
    PALETTE[models.findIndex((m) => m.id === id) % PALETTE.length] || "#4f9cf9";

  const t = $derived(Math.max(days, 0) / 365);

  const curveLines = $derived<SmileCurveLine[]>(
    models
      .filter((m) => visible[m.id] && fits[m.id])
      .map((m) => ({
        name: m.name,
        curve: fits[m.id].curve,
        color: colorOf(m.id),
        active: m.id === activeModel,
      })),
  );

  async function calibrate() {
    error = null;
    try {
      const results = await Promise.all(
        models.map((m) => ipc.smileFit({ model: m.id, points, forward, t })),
      );
      const next: Record<string, SmileFitDto> = {};
      models.forEach((m, i) => (next[m.id] = results[i]));
      fits = next;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
    try {
      models = await ipc.listSmileModels();
    } catch {
      models = [{ id: "svi", name: "SVI" }];
    }
    visible = Object.fromEntries(models.map((m) => [m.id, true]));
    if (!models.some((m) => m.id === activeModel)) activeModel = models[0]?.id ?? "svi";
    await calibrate();
  });
</script>

<div class="smile-grid">
  <section class="panel">
    <header class="panel-head">
      <span>Улыбка волатильности</span>
      <div class="toggles">
        {#each models as m (m.id)}
          <button
            class="chip-btn"
            class:active={visible[m.id]}
            style:--chip={colorOf(m.id)}
            onclick={() => (visible = { ...visible, [m.id]: !visible[m.id] })}>{m.name}</button
          >
        {/each}
      </div>
    </header>
    <div class="panel-body chart-body">
      <div class="controls">
        <label>Форвард F<input class="ctl-sm" type="number" bind:value={forward} step="1" /></label>
        <label>Дней до эксп.<input class="ctl-sm" type="number" bind:value={days} step="1" /></label>
        <button class="btn-primary" onclick={calibrate}>Калибровать</button>
      </div>
      {#if error}<div class="ph-box error">{error}</div>{/if}
      <div class="chart"><SmileChart {points} fits={curveLines} /></div>
    </div>
  </section>

  <section class="panel">
    <header class="panel-head">Параметры моделей</header>
    <div class="panel-body cards">
      {#each models as m (m.id)}
        {@const f = fits[m.id]}
        <div class="card" class:dim={!visible[m.id]}>
          <div class="card-head">
            <span class="dot" style:background={colorOf(m.id)}></span>
            <span class="name">{m.name}</span>
            <span class="rmse">{f ? "RMSE " + (f.rmse * 100).toFixed(2) + "%" : "…"}</span>
            <button
              class="seg-btn active-btn"
              class:active={activeModel === m.id}
              onclick={() => (activeModel = m.id)}
            >
              {activeModel === m.id ? "активна" : "выбрать"}
            </button>
          </div>
          {#if f}
            <div class="params">
              {#each f.params as p (p.name)}
                <span><span class="k">{p.name}</span> = {p.value.toFixed(4)}</span>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
      <div class="note">
        Размер точки ∝ открытому интересу (OI). Активная модель задаёт «рабочую» улыбку доски.
      </div>
    </div>
  </section>
</div>

<style>
  .smile-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(280px, 320px);
    gap: 8px;
    height: 100%;
    min-height: 0;
  }
  .toggles {
    margin-left: auto;
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
    text-transform: none;
    letter-spacing: 0;
  }
  .chip-btn.active {
    border-color: var(--chip);
    color: var(--chip);
    background: color-mix(in srgb, var(--chip) 15%, transparent);
  }
  .chart-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .controls {
    display: flex;
    gap: 12px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    flex-direction: column;
    font-size: 11px;
    color: var(--text-dim);
    gap: 3px;
  }
  .chart {
    flex: 1;
    min-height: 380px;
  }
  .cards {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .card.dim {
    opacity: 0.45;
  }
  .card-head {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 9px;
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 2px;
  }
  .name {
    font-weight: 600;
    font-size: 12px;
  }
  .rmse {
    margin-left: auto;
    font-size: 11px;
    color: var(--text-dim);
  }
  .active-btn {
    font-size: 11px;
    border: 1px solid var(--border);
  }
  .params {
    padding: 7px 9px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 11px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .params .k {
    color: var(--text);
  }
  .note {
    color: var(--text-dim);
    font-size: 11px;
    line-height: 1.5;
    margin-top: 2px;
  }
</style>
