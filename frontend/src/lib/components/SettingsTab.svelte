<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "../ipc";
  import {
    loadSettings,
    saveSettings,
    type Settings,
    type LlmProvider,
    type DataSourceId,
    type PricingModelId,
    type SmileModelId,
  } from "../settings";

  // Вкладка «Настройки» (Фазы 10–12): отображение, паспорт MOEX ALGO,
  // конструктор правил Key Activity, LLM-резюме, историзация, опционы.
  // Секреты (ключ провайдера, токен ALGOPACK) в UI не хранятся — только флаг
  // наличия; сами значения живут в ОС-keyring / .env на стороне ядра.

  let s = $state<Settings>(loadSettings());

  function commit() {
    saveSettings(s);
  }

  // ── Rule builder (Key Activity) ────────────────────────────────────────────
  type Cond = {
    conn?: "AND" | "OR" | "NOT";
    metric: string;
    op: string;
    threshold: string;
    scope: string;
  };
  type Rule = { id: string; name: string; severity: "high" | "med" | "low"; conds: Cond[] };

  const RULES_KEY = "market-terminal:ka-rules";
  const metricOpts = [
    ["volume", "Объём"],
    ["disb", "Дисбаланс (disb)"],
    ["oi", "Открытый интерес (OI)"],
    ["hi2", "Концентрация HI2"],
    ["spread", "Спред"],
    ["price", "Изменение цены"],
    ["turnover", "Оборот"],
  ];
  const opOpts = [
    ["gt", "> больше"],
    ["lt", "< меньше"],
    ["ge", "≥ не меньше"],
    ["le", "≤ не больше"],
    ["cross", "пересечение"],
    ["zscore", "z-score > k"],
  ];
  const scopeOpts = [
    ["ticker", "Тикер"],
    ["set", "Набор тикеров"],
    ["market", "Весь рынок"],
    ["class", "Класс актива"],
  ];
  const conns: ["AND" | "OR" | "NOT", string][] = [
    ["AND", "И"],
    ["OR", "ИЛИ"],
    ["NOT", "НЕ"],
  ];

  function defaultRules(): Rule[] {
    return [
      { id: "r1", name: "Топ по обороту", severity: "high", conds: [{ metric: "turnover", op: "gt", threshold: "20", scope: "market" }] },
      {
        id: "r2",
        name: "Аномальный объём + разворот дисбаланса",
        severity: "high",
        conds: [
          { metric: "volume", op: "zscore", threshold: "3", scope: "set" },
          { conn: "AND", metric: "disb", op: "cross", threshold: "0", scope: "set" },
        ],
      },
      {
        id: "r3",
        name: "Экстремум концентрации",
        severity: "med",
        conds: [
          { metric: "hi2", op: "gt", threshold: "0.35", scope: "ticker" },
          { conn: "OR", metric: "oi", op: "zscore", threshold: "2.5", scope: "ticker" },
        ],
      },
    ];
  }

  let rules = $state<Rule[]>([]);
  let showJson = $state(false);
  let saved = $state(false);
  let backendRuleNames = $state<string[]>([]);

  const rulesJson = $derived(JSON.stringify(rules, null, 2));

  function loadRules() {
    if (typeof localStorage === "undefined") return defaultRules();
    try {
      const raw = localStorage.getItem(RULES_KEY);
      if (raw) return JSON.parse(raw) as Rule[];
    } catch {
      /* corrupt — fall through */
    }
    return defaultRules();
  }

  function severityFromWeight(w: number): Rule["severity"] {
    return w >= 0.9 ? "high" : w >= 0.7 ? "med" : "low";
  }

  function addRule() {
    rules = [
      ...rules,
      {
        id: "r" + Date.now(),
        name: "Новое правило",
        severity: "med",
        conds: [{ metric: "volume", op: "gt", threshold: "", scope: "ticker" }],
      },
    ];
  }
  function removeRule(i: number) {
    rules = rules.filter((_, idx) => idx !== i);
  }
  function addCond(ri: number) {
    rules[ri].conds = [
      ...rules[ri].conds,
      { conn: "AND", metric: "volume", op: "gt", threshold: "", scope: "ticker" },
    ];
    rules = [...rules];
  }
  function removeCond(ri: number, ci: number) {
    rules[ri].conds = rules[ri].conds.filter((_, idx) => idx !== ci);
    if (rules[ri].conds.length === 0) rules = rules.filter((_, idx) => idx !== ri);
    else rules = [...rules];
  }
  function resetRules() {
    rules = defaultRules();
    saved = false;
  }
  function saveRules() {
    if (typeof localStorage !== "undefined") {
      try {
        localStorage.setItem(RULES_KEY, JSON.stringify(rules));
      } catch {
        /* ignore */
      }
    }
    saved = true;
    setTimeout(() => (saved = false), 1600);
  }

  const sevDot = (sev: string) =>
    sev === "high" ? "var(--down)" : sev === "med" ? "#f5a623" : "var(--text-dim)";

  // ── passport / watchlist chips ─────────────────────────────────────────────
  const marketList: [keyof Settings["markets"], string][] = [
    ["eq", "Акции"],
    ["fo", "Фьючерсы"],
    ["fx", "Валюта"],
  ];
  const watchTickers = ["SBER", "GAZP", "LKOH", "GMKN", "ROSN", "VTBR", "YDEX", "MGNT"];

  function toggleMarket(id: keyof Settings["markets"]) {
    s.markets = { ...s.markets, [id]: !s.markets[id] };
    commit();
  }
  function toggleWatch(tk: string) {
    s.watchlist = { ...s.watchlist, [tk]: !s.watchlist[tk] };
    commit();
  }

  const providers: [LlmProvider, string][] = [
    ["openrouter", "OpenRouter"],
    ["anthropic", "Anthropic"],
    ["openai", "OpenAI"],
  ];
  const dataSources: [DataSourceId, string][] = [
    ["finam", "Finam"],
    ["moex_algo", "MOEX ALGO"],
  ];
  const pricingModels: [PricingModelId, string][] = [
    ["black76", "Black-76"],
    ["bachelier", "Bachelier"],
  ];
  const smileModels: [SmileModelId, string][] = [
    ["moex", "MOEX"],
    ["sabr", "SABR"],
    ["svi", "SVI"],
    ["kalen", "Каленкович"],
  ];
  const periodList: [Settings["defaultPeriod"], string][] = [
    ["1h", "1ч"],
    ["1d", "1д"],
    ["1w", "1н"],
    ["1m", "1м"],
    ["3m", "3м"],
  ];

  onMount(async () => {
    rules = loadRules();
    try {
      const backend = await ipc.keyActivityRules();
      backendRuleNames = backend.map((r) => r.name);
      // Если пользователь ещё не сохранял свои правила — засеять из доменных.
      if (typeof localStorage !== "undefined" && !localStorage.getItem(RULES_KEY)) {
        rules = backend.map((r, i) => ({
          id: r.id || "r" + i,
          name: r.name,
          severity: severityFromWeight(r.weight),
          conds: [{ metric: "volume", op: "gt", threshold: "", scope: "ticker" }],
        }));
      }
    } catch {
      /* оффлайн — остаёмся на локальных правилах */
    }
  });
</script>

<div class="settings-tab">
  <!-- Отображение -->
  <section class="panel">
    <header class="panel-head">Отображение</header>
    <div class="panel-body row">
      <label class="field">
        <span class="field-label">Глубина стакана</span>
        <input class="ctl" type="number" min="1" max="50" bind:value={s.domDepth} onchange={commit} />
      </label>
      <label class="field">
        <span class="field-label">Размер ленты</span>
        <input class="ctl" type="number" min="10" max="500" bind:value={s.tapeLimit} onchange={commit} />
      </label>
      <label class="field">
        <span class="field-label">Лимит лидеров</span>
        <input class="ctl" type="number" min="1" max="100" bind:value={s.topMoversLimit} onchange={commit} />
      </label>
    </div>
  </section>

  <!-- Паспорт MOEX ALGO -->
  <section class="panel">
    <header class="panel-head">MOEX ALGO / Passport</header>
    <div class="panel-body col">
      <div class="passport-line">
        <span>Подключение MOEX ALGOPACK</span>
        <span class="key-badge">секрет задан: да</span>
        <span class="hint">env → .env (MOEX_ALGO_API) → keyring · значение не хранится в UI</span>
      </div>
      <div class="row">
        <div>
          <div class="field-label">Рынки</div>
          <div class="chips">
            {#each marketList as [id, label] (id)}
              <button class="chip-btn" class:active={s.markets[id]} onclick={() => toggleMarket(id)}>
                {label}
              </button>
            {/each}
          </div>
        </div>
        <div class="grow">
          <div class="field-label">Вотчлист ALGOPACK</div>
          <div class="chips">
            {#each watchTickers as tk (tk)}
              <button class="chip-btn" class:active={!!s.watchlist[tk]} onclick={() => toggleWatch(tk)}>
                {tk}
              </button>
            {/each}
          </div>
        </div>
      </div>
    </div>
  </section>

  <!-- Конструктор правил -->
  <section class="panel">
    <header class="panel-head">
      <span>Конструктор правил · Key Activity</span>
      <div class="head-actions">
        <button class="btn-ghost" onclick={() => (showJson = !showJson)}>
          {showJson ? "Скрыть JSON" : "Импорт/экспорт JSON"}
        </button>
        <button class="btn-ghost" onclick={resetRules}>Сбросить к дефолтам</button>
        <button class="btn-primary" onclick={saveRules}>{saved ? "✓ Сохранено" : "Сохранить"}</button>
      </div>
    </header>
    <div class="panel-body col">
      {#if backendRuleNames.length > 0}
        <div class="hint">
          Доменные правила ядра (<code>domain::algo</code>): {backendRuleNames.join(" · ")}.
        </div>
      {/if}
      {#each rules as rule, ri (rule.id)}
        <div class="rule">
          <div class="rule-head">
            <span class="dot" style:background={sevDot(rule.severity)}></span>
            <input class="ctl-sm rule-name" bind:value={rule.name} />
            <span class="hint">важность:</span>
            <select class="ctl-sm" bind:value={rule.severity}>
              <option value="high">Высокая</option>
              <option value="med">Средняя</option>
              <option value="low">Низкая</option>
            </select>
            <button class="btn-ghost danger head-actions" onclick={() => removeRule(ri)}>
              Удалить правило
            </button>
          </div>
          <div class="rule-body">
            <div class="cond-caption">ЕСЛИ выполнены условия:</div>
            {#each rule.conds as cond, ci (ci)}
              <div class="cond">
                {#if ci > 0}
                  <div class="seg-wrap sm">
                    {#each conns as [id, label] (id)}
                      <button
                        class="seg-btn"
                        class:active={(cond.conn ?? "AND") === id}
                        onclick={() => {
                          cond.conn = id;
                          rules = [...rules];
                        }}>{label}</button
                      >
                    {/each}
                  </div>
                {:else}
                  <span class="cond-lead">метрика</span>
                {/if}
                <select class="ctl-sm" bind:value={cond.metric}>
                  {#each metricOpts as [id, label] (id)}<option value={id}>{label}</option>{/each}
                </select>
                <select class="ctl-sm" bind:value={cond.op}>
                  {#each opOpts as [id, label] (id)}<option value={id}>{label}</option>{/each}
                </select>
                <input class="ctl-sm thr" placeholder="порог" bind:value={cond.threshold} />
                <span class="hint">область</span>
                <select class="ctl-sm" bind:value={cond.scope}>
                  {#each scopeOpts as [id, label] (id)}<option value={id}>{label}</option>{/each}
                </select>
                <button class="btn-ghost danger x" onclick={() => removeCond(ri, ci)}>×</button>
              </div>
            {/each}
            <div class="cond-foot">
              <button class="btn-ghost" onclick={() => addCond(ri)}>+ Условие</button>
              <span class="hint">ТО пометить событие правилом «{rule.name}»</span>
            </div>
          </div>
        </div>
      {/each}
      <button class="btn-ghost self-start" onclick={addRule}>+ Новое правило</button>
      {#if showJson}
        <div class="json-box">
          <div class="field-label">JSON конфигурация (импорт/экспорт)</div>
          <textarea readonly>{rulesJson}</textarea>
        </div>
      {/if}
    </div>
  </section>

  <!-- LLM -->
  <section class="panel">
    <header class="panel-head">LLM · ИИ-резюме</header>
    <div class="panel-body col">
      <div class="row">
        <div>
          <div class="field-label">Провайдер</div>
          <div class="seg-wrap">
            {#each providers as [id, label] (id)}
              <button
                class="seg-btn"
                class:active={s.llmProvider === id}
                onclick={() => {
                  s.llmProvider = id;
                  commit();
                }}>{label}</button
              >
            {/each}
          </div>
        </div>
        <label class="field grow">
          <span class="field-label">Модель</span>
          <input class="ctl" bind:value={s.llmModel} onchange={commit} />
        </label>
      </div>
      <div class="row ac">
        <div class="passport-line">
          <span>Ключ провайдера</span>
          <span class="key-badge" class:off={!s.llmHasKey}>
            {s.llmHasKey ? "секрет задан: да" : "секрет задан: нет"}
          </span>
          <button
            class="btn-ghost"
            onclick={() => {
              s.llmHasKey = !s.llmHasKey;
              commit();
            }}>переключить</button
          >
        </div>
        <label class="field">
          <span class="field-label">Лимит токенов</span>
          <input class="ctl" type="number" bind:value={s.llmTokenLimit} onchange={commit} />
        </label>
        <label class="toggle">
          <button
            class="switch"
            class:on={s.llmAuto}
            aria-pressed={s.llmAuto}
            aria-label="Авто-резюме"
            onclick={() => {
              s.llmAuto = !s.llmAuto;
              commit();
            }}><span class="knob"></span></button
          >
          <span>Авто-резюме</span>
        </label>
      </div>
      <div>
        <div class="field-label">Период анализа по умолчанию</div>
        <div class="seg-wrap">
          {#each periodList as [id, label] (id)}
            <button
              class="seg-btn"
              class:active={s.defaultPeriod === id}
              onclick={() => {
                s.defaultPeriod = id;
                commit();
              }}>{label}</button
            >
          {/each}
        </div>
      </div>
    </div>
  </section>

  <!-- Данные -->
  <section class="panel">
    <header class="panel-head">Данные / Историзация</header>
    <div class="panel-body row ac">
      <div>
        <div class="field-label">Источник по умолчанию</div>
        <div class="seg-wrap">
          {#each dataSources as [id, label] (id)}
            <button
              class="seg-btn"
              class:active={s.dataSource === id}
              onclick={() => {
                s.dataSource = id;
                commit();
              }}>{label}</button
            >
          {/each}
        </div>
      </div>
      <label class="field grow">
        <span class="field-label">Директория хранения</span>
        <input class="ctl" bind:value={s.dataDir} onchange={commit} />
      </label>
      <label class="field">
        <span class="field-label">Параллелизм</span>
        <input class="ctl" type="number" min="1" max="16" bind:value={s.concurrency} onchange={commit} />
      </label>
    </div>
  </section>

  <!-- Опционы -->
  <section class="panel">
    <header class="panel-head">Опционы</header>
    <div class="panel-body row ac">
      <div>
        <div class="field-label">Модель ценообразования</div>
        <div class="seg-wrap">
          {#each pricingModels as [id, label] (id)}
            <button
              class="seg-btn"
              class:active={s.pricingModel === id}
              onclick={() => {
                s.pricingModel = id;
                commit();
              }}>{label}</button
            >
          {/each}
        </div>
      </div>
      <label class="field">
        <span class="field-label">Ставка r, %</span>
        <input class="ctl" type="number" step="0.5" bind:value={s.rate} onchange={commit} />
      </label>
      <div>
        <div class="field-label">Модель улыбки по умолчанию</div>
        <div class="seg-wrap">
          {#each smileModels as [id, label] (id)}
            <button
              class="seg-btn"
              class:active={s.defaultSmile === id}
              onclick={() => {
                s.defaultSmile = id;
                commit();
              }}>{label}</button
            >
          {/each}
        </div>
      </div>
    </div>
  </section>
</div>

<style>
  .settings-tab {
    max-width: 960px;
    margin: 0 auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .row {
    display: flex;
    gap: 20px;
    flex-wrap: wrap;
  }
  .row.ac {
    align-items: flex-end;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field .field-label {
    margin: 0;
  }
  .grow {
    flex: 1;
    min-width: 220px;
  }
  .head-actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
    text-transform: none;
    letter-spacing: 0;
  }
  .passport-line {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    flex-wrap: wrap;
  }
  .hint {
    color: var(--text-dim);
    font-size: 11px;
  }
  code {
    background: var(--bg-elev);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 11px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  /* rule builder */
  .rule {
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg);
  }
  .rule-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .rule-name {
    font-weight: 600;
    width: 230px;
    max-width: 100%;
  }
  .rule-body {
    padding: 9px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .cond-caption {
    font-size: 11px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }
  .cond {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .cond-lead {
    width: 96px;
    font-size: 11px;
    color: var(--text-dim);
  }
  .seg-wrap.sm {
    padding: 1px;
  }
  .thr {
    width: 80px;
    text-align: right;
  }
  .x {
    padding: 1px 8px;
  }
  .cond-foot {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 2px;
  }
  .self-start {
    align-self: flex-start;
  }
  .json-box {
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    overflow: hidden;
  }
  .json-box .field-label {
    padding: 7px 10px;
    margin: 0;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev);
  }
  textarea {
    width: 100%;
    height: 180px;
    background: var(--bg);
    color: var(--text-dim);
    border: none;
    padding: 10px;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 11px;
    line-height: 1.5;
    resize: vertical;
    box-sizing: border-box;
  }
  /* toggle switch */
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .switch {
    appearance: none;
    border: none;
    width: 38px;
    height: 21px;
    border-radius: 11px;
    cursor: pointer;
    position: relative;
    background: var(--border);
    padding: 0;
  }
  .switch.on {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: #fff;
    transition: left 0.15s;
  }
  .switch.on .knob {
    left: 19px;
  }
</style>
