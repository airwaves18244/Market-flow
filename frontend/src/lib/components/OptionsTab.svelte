<script lang="ts">
  import OptionCalculator from "./OptionCalculator.svelte";
  import SmileView from "./SmileView.svelte";
  import StrategyBuilder from "./StrategyBuilder.svelte";

  // Вкладка «Опционы»: калькулятор · улыбка · конструктор стратегий.
  const sections = [
    { id: "calc", label: "Калькулятор" },
    { id: "smile", label: "Улыбка" },
    { id: "strategy", label: "Конструктор стратегий" },
  ];
  let active = $state("calc");
</script>

<div class="options">
  <nav class="subnav">
    {#each sections as s (s.id)}
      <button class="seg" class:active={s.id === active} onclick={() => (active = s.id)}>
        {s.label}
      </button>
    {/each}
  </nav>

  <div class="body">
    {#if active === "calc"}
      <OptionCalculator />
    {:else if active === "smile"}
      <SmileView />
    {:else if active === "strategy"}
      <StrategyBuilder />
    {/if}
  </div>
</div>

<style>
  .options {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
  }
  .subnav {
    display: flex;
    gap: 4px;
  }
  .seg {
    appearance: none;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-dim);
    font-size: 12px;
    padding: 6px 12px;
    cursor: pointer;
  }
  .seg.active {
    color: #fff;
    background: var(--accent);
    border-color: var(--accent);
  }
</style>
