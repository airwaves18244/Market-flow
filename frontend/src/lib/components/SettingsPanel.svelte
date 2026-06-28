<script lang="ts">
  import type { Settings } from "../settings";

  // Панель настроек представления. Изменения уходят наверх через `onChange`
  // (родитель сохраняет в localStorage и перезагружает зависимые данные).
  let {
    settings,
    onChange,
  }: {
    settings: Settings;
    onChange: (s: Settings) => void;
  } = $props();

  function update(patch: Partial<Settings>) {
    onChange({ ...settings, ...patch });
  }

  function num(e: Event): number {
    return Number((e.target as HTMLInputElement).value);
  }
</script>

<div class="settings">
  <label>
    <span>Лента сделок, строк</span>
    <input
      type="number"
      min="10"
      max="500"
      value={settings.tapeLimit}
      onchange={(e) => update({ tapeLimit: num(e) })}
    />
  </label>
  <label>
    <span>Глубина стакана</span>
    <input
      type="number"
      min="1"
      max="50"
      value={settings.domDepth}
      onchange={(e) => update({ domDepth: num(e) })}
    />
  </label>
  <label>
    <span>Топ-движения, строк</span>
    <input
      type="number"
      min="1"
      max="100"
      value={settings.topMoversLimit}
      onchange={(e) => update({ topMoversLimit: num(e) })}
    />
  </label>
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 12px;
  }
  label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  span {
    color: var(--text-dim);
  }
  input {
    width: 80px;
    background: var(--bg-elev);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: 12px;
  }
</style>
