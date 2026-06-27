<script lang="ts">
  import type { TriggeredAlertDto } from "../types";

  let { alerts = [] }: { alerts: TriggeredAlertDto[] } = $props();

  const severityLabel: Record<string, string> = {
    info: "инфо",
    warning: "внимание",
    critical: "критично",
  };
</script>

{#if alerts.length > 0}
  <ul class="alerts">
    {#each alerts as a (a.ruleId)}
      <li class="alert {a.severity}">
        <span class="badge">{severityLabel[a.severity] ?? a.severity}</span>
        <span class="sym">{a.symbol}</span>
        <span class="msg">{a.message}</span>
      </li>
    {/each}
  </ul>
{:else}
  <div class="empty">Активных алёртов нет</div>
{/if}

<style>
  .alerts {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .alert {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    border-left: 3px solid var(--border);
    background: var(--bg-elev);
    font-size: 12px;
  }
  .alert.info {
    border-left-color: #3b82f6;
  }
  .alert.warning {
    border-left-color: #d29922;
  }
  .alert.critical {
    border-left-color: var(--down, #f85149);
  }
  .badge {
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.4px;
    color: var(--text-dim);
    min-width: 64px;
  }
  .sym {
    font-weight: 600;
    color: var(--accent);
    min-width: 90px;
  }
  .msg {
    color: var(--text);
  }
  .empty {
    padding: 16px;
    color: var(--text-dim);
    text-align: center;
  }
</style>
