<script lang="ts">
  import { onMount } from "svelte";
  import Panel from "./Panel.svelte";
  import InstrumentList from "./InstrumentList.svelte";
  import DeltaChart from "./DeltaChart.svelte";
  import { ipc } from "../ipc";
  import { fmtRu } from "../format";
  import type {
    BarPoint,
    FootprintBarDto,
    InstrumentDto,
    RobotConfigInput,
    RobotSignalDto,
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

  const FULL_RANGE = Number.MAX_SAFE_INTEGER;

  let bars = $state<BarPoint[]>([]);
  let footprint = $state<FootprintBarDto[]>([]);
  let signals = $state<RobotSignalDto[]>([]);
  let error = $state<string | null>(null);

  // Переключатели детекторов (роботов).
  let sameLot = $state(true);
  let iceberg = $state(true);
  let absorption = $state(true);

  const robotMeta: Record<string, { label: string; color: string }> = {
    same_lot: { label: "Равные лоты", color: "#f5b942" },
    iceberg: { label: "Айсберг", color: "#4f9cf9" },
    absorption: { label: "Поглощение", color: "#ef5350" },
  };

  // Footprint бара, на котором наибольшая |дельта| — для числовой лесенки.
  const focusBar = $derived(
    footprint.length
      ? footprint.reduce((a, b) => (Math.abs(b.delta) > Math.abs(a.delta) ? b : a))
      : null,
  );

  async function load() {
    error = null;
    const config: RobotConfigInput = {
      sameLotEnabled: sameLot,
      icebergEnabled: iceberg,
      absorptionEnabled: absorption,
    };
    try {
      [bars, footprint, signals] = await Promise.all([
        ipc.bars(selected, "d1", 0, FULL_RANGE),
        ipc.deltaFootprint(selected, "d1", 0, FULL_RANGE, 0.01),
        ipc.robotScan(selected, 0, FULL_RANGE, config),
      ]);
    } catch (e) {
      error = String(e);
    }
  }

  // Перезагрузка при смене инструмента или набора включённых роботов.
  $effect(() => {
    void selected;
    void sameLot;
    void iceberg;
    void absorption;
    load();
  });

  onMount(load);

</script>

<main class="grid">
  <Panel title={`Дельта / footprint — ${selected}`}>
    {#if error}<div class="error">{error}</div>{/if}
    <DeltaChart {bars} {footprint} {signals} />
  </Panel>

  <Panel title="Инструменты">
    <InstrumentList items={instruments} {selected} onSelect={onSelect} />
  </Panel>

  <Panel title="Роботы — детекторы">
    <div class="robots">
      <label><input type="checkbox" bind:checked={sameLot} /> <span style="color:{robotMeta.same_lot.color}">●</span> Равные лоты</label>
      <label><input type="checkbox" bind:checked={iceberg} /> <span style="color:{robotMeta.iceberg.color}">●</span> Айсберг</label>
      <label><input type="checkbox" bind:checked={absorption} /> <span style="color:{robotMeta.absorption.color}">●</span> Поглощение</label>
    </div>
    <table>
      <thead>
        <tr><th>Робот</th><th>Время</th><th class="num">Цена</th><th>Заметка</th></tr>
      </thead>
      <tbody>
        {#each signals as s (s.kind + s.ts + s.price)}
          <tr>
            <td><span style="color:{robotMeta[s.kind]?.color ?? '#8b98a9'}">●</span> {robotMeta[s.kind]?.label ?? s.kind}</td>
            <td>{new Date(s.ts * 1000).toLocaleDateString("ru-RU")}</td>
            <td class="num">{fmtRu(s.price)}</td>
            <td>{s.note}</td>
          </tr>
        {/each}
        {#if signals.length === 0}
          <tr><td colspan="4" class="empty">Сигналов не найдено.</td></tr>
        {/if}
      </tbody>
    </table>
  </Panel>

  <Panel title={focusBar ? `Footprint бара ${new Date(focusBar.ts * 1000).toLocaleDateString("ru-RU")}` : "Footprint"}>
    {#if focusBar}
      <table>
        <thead>
          <tr><th class="num">Цена</th><th class="num">Bid</th><th class="num">Ask</th><th class="num">Δ</th></tr>
        </thead>
        <tbody>
          {#each [...focusBar.cells].sort((a, b) => b.price - a.price) as c (c.price)}
            <tr>
              <td class="num">{fmtRu(c.price)}</td>
              <td class="num down">{fmtRu(c.bidVolume)}</td>
              <td class="num up">{fmtRu(c.askVolume)}</td>
              <td class="num" class:up={c.delta > 0} class:down={c.delta < 0}>{fmtRu(c.delta)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="empty">Нет тиковых сделок для footprint.</p>
    {/if}
  </Panel>
</main>

<style>
  .robots {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-bottom: 8px;
    font-size: 13px;
  }
  .robots label {
    display: flex;
    align-items: center;
    gap: 5px;
    cursor: pointer;
  }
  .empty {
    color: var(--text-dim);
    font-size: 13px;
    padding: 8px;
    text-align: center;
  }
</style>
