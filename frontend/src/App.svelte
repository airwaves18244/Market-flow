<script lang="ts">
  import { equityDashboard, flowSeries } from "./lib/ipc";
  import type { EquityDashboard, FlowPoint } from "./lib/types";
  import SectorPanel from "./lib/panels/SectorPanel.svelte";
  import MoversPanel from "./lib/panels/MoversPanel.svelte";
  import FlowPanel from "./lib/panels/FlowPanel.svelte";

  const DAY = 86_400;
  const toTs = Math.floor(Date.now() / 1000);
  const fromTs = toTs - 30 * DAY;

  let dashboard = $state<EquityDashboard | null>(null);
  let flow = $state<FlowPoint[]>([]);
  let focus = $state("SBER@MISX");
  let error = $state<string | null>(null);

  // TODO (§ 3.4): заменить статичную сетку на докуемые панели dockview-core.
  async function load() {
    try {
      const d = await equityDashboard(fromTs, toTs);
      dashboard = d;
      focus = d.top_movers[0]?.symbol ?? focus;
      flow = await flowSeries(focus, fromTs, toTs);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  $effect(() => {
    void load();
  });
</script>

<header>
  <h1>Market Terminal</h1>
  <span class="subtitle">Акции · обороты и денежные потоки</span>
</header>

{#if error}
  <p class="error">Ошибка загрузки: {error}</p>
{/if}

<main class="grid">
  {#if dashboard}
    <SectorPanel sectors={dashboard.sectors} />
    <MoversPanel movers={dashboard.top_movers} />
    <FlowPanel {flow} symbol={focus} />
  {:else}
    <p class="loading">Загрузка…</p>
  {/if}
</main>
