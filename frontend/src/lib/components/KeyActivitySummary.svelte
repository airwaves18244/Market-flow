<script lang="ts">
  import Panel from "./Panel.svelte";
  import type { KeyActivitySummaryDto } from "../types";

  // Панель «ИТОГО»: ИИ-резюме (или локальный свод без LLM-ключа).
  let {
    summary = null,
    loading = false,
    onRefresh,
  }: {
    summary: KeyActivitySummaryDto | null;
    loading: boolean;
    onRefresh: () => void;
  } = $props();
</script>

<Panel title="ИТОГО (ИИ-резюме)">
  <div class="total">
    <div class="head">
      <span class="badge">{summary?.fallback ? "локальный свод" : "LLM"}</span>
      {#if summary}<span class="meta">период {summary.period} · {summary.rowCount} сигналов</span>{/if}
      <button onclick={onRefresh} disabled={loading}>{loading ? "…" : "Обновить"}</button>
    </div>
    {#if loading}
      <div class="skeleton">генерация свода…</div>
    {:else if summary}
      <pre class="text">{summary.text}</pre>
      {#if summary.fallback}
        <div class="hint">
          Показан локальный свод. Добавьте LLM-ключ (OpenRouter/Anthropic/OpenAI) в Настройках
          для развёрнутого ИИ-анализа.
        </div>
      {/if}
    {:else}
      <div class="empty">нет данных</div>
    {/if}
  </div>
</Panel>

<style>
  .total {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .badge {
    background: rgba(79, 156, 249, 0.18);
    color: #4f9cf9;
    border-radius: 10px;
    padding: 1px 8px;
    font-size: 11px;
  }
  .meta {
    font-size: 11px;
    color: var(--text-dim);
  }
  button {
    margin-left: auto;
    background: var(--accent);
    border: none;
    color: #fff;
    border-radius: 4px;
    padding: 4px 12px;
    font-size: 12px;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .text {
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 13px;
    line-height: 1.5;
    margin: 0;
    color: var(--text);
  }
  .hint {
    font-size: 11px;
    color: var(--text-dim);
    border-top: 1px solid var(--border);
    padding-top: 6px;
  }
  .skeleton,
  .empty {
    font-size: 12px;
    color: var(--text-dim);
  }
</style>
