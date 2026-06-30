<script lang="ts">
  // Верхняя навигация по модулям терминала.
  let {
    tabs,
    active,
    onSelect,
  }: {
    tabs: { id: string; label: string }[];
    active: string;
    onSelect: (id: string) => void;
  } = $props();
</script>

<nav class="tabbar">
  {#each tabs as tab (tab.id)}
    <button class="tab" class:active={tab.id === active} onclick={() => onSelect(tab.id)}>
      {tab.label}
    </button>
  {/each}
</nav>

<style>
  .tabbar {
    display: flex;
    gap: 2px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-panel);
  }
  .tab {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 500;
    padding: 9px 16px;
    cursor: pointer;
    border-bottom: 2px solid transparent;
  }
  .tab:hover {
    color: var(--text);
  }
  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
</style>
