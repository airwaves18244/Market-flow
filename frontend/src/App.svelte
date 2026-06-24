<script lang="ts">
  import { onMount } from "svelte";
  import type { DockviewApi } from "dockview-core";
  import { store } from "./lib/store.svelte";
  import { buildLayout } from "./lib/dock";

  let api: DockviewApi | undefined;

  // Действие строит докуемый layout в контейнере; панели монтируются dockview.
  function dock(node: HTMLElement) {
    api = buildLayout(node);
    return {
      destroy() {
        api?.dispose();
      },
    };
  }

  onMount(() => {
    void store.load();
  });
</script>

<header>
  <h1>Market Terminal</h1>
  <span class="subtitle">Акции · обороты и денежные потоки</span>
  {#if store.loading}<span class="badge">загрузка…</span>{/if}
  {#if store.error}<span class="badge error">{store.error}</span>{/if}
</header>

<main class="dock-host" use:dock></main>
