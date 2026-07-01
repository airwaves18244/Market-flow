<script lang="ts">
  import { onMount } from "svelte";
  import TabBar from "./lib/components/TabBar.svelte";
  import Overview from "./lib/components/Overview.svelte";
  import DeltaView from "./lib/components/DeltaView.svelte";
  import TradePanel from "./lib/components/TradePanel.svelte";
  import Backtester from "./lib/components/Backtester.svelte";
  import OptionsTab from "./lib/components/OptionsTab.svelte";
  import MoexAlgoTab from "./lib/components/MoexAlgoTab.svelte";
  import { ipc } from "./lib/ipc";
  import type { InstrumentDto } from "./lib/types";

  const tabs = [
    { id: "overview", label: "Обзор" },
    { id: "delta", label: "Delta" },
    { id: "trade", label: "Торговля" },
    { id: "backtest", label: "Бэктест" },
    { id: "moexalgo", label: "MOEX ALGO" },
    { id: "options", label: "Опционы" },
  ];

  let instruments = $state<InstrumentDto[]>([]);
  let selected = $state("SBER@MISX");
  let activeTab = $state("overview");
  let error = $state<string | null>(null);

  const select = (symbol: string) => (selected = symbol);

  onMount(async () => {
    try {
      instruments = await ipc.instruments();
      if (instruments.length > 0) selected = instruments[0].symbol;
    } catch (e) {
      error = String(e);
    }
  });
</script>

<div class="app">
  <header class="app-header">
    <h1>Market Terminal</h1>
    <span class="sub">Обзор · Delta · Торговля · Бэктест · MOEX ALGO · Опционы</span>
    <span class="status">{instruments.length} инструментов · {selected}</span>
  </header>

  <TabBar {tabs} active={activeTab} onSelect={(t) => (activeTab = t)} />

  {#if error}
    <div class="error">Ошибка загрузки: {error}</div>
  {/if}

  {#if activeTab === "overview"}
    <Overview {instruments} {selected} onSelect={select} />
  {:else if activeTab === "delta"}
    <DeltaView {instruments} {selected} onSelect={select} />
  {:else if activeTab === "trade"}
    <TradePanel {instruments} {selected} onSelect={select} />
  {:else if activeTab === "backtest"}
    <Backtester {instruments} {selected} />
  {:else if activeTab === "moexalgo"}
    <MoexAlgoTab />
  {:else if activeTab === "options"}
    <OptionsTab />
  {/if}
</div>
