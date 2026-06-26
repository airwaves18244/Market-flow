<script lang="ts">
  import type { Snippet } from "svelte";

  // Контейнер докуемой панели: заголовок + область контента.
  let { title, children }: { title: string; children: Snippet } = $props();
</script>

<section class="panel">
  <header class="panel-head">{title}</header>
  <div class="panel-body">
    {@render children()}
  </div>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-panel);
    overflow: hidden;
  }
  .panel-head {
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.4px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .panel-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px;
  }
</style>
