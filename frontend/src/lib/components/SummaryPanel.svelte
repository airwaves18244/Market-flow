<script lang="ts">
  import type { RegimeSignalDto, Regime } from "../types";
  import { assetLabel, assetColor } from "../assetClass";

  // Вкладка «Сводка»: режим рынка «куда идут большие деньги».
  // Числовой сигнал (режим, уверенность, нетто-потоки) приходит из ядра
  // (`ipc.summary`); текстовые пояснения собираются здесь по коду режима.
  let { signal }: { signal: RegimeSignalDto } = $props();

  const REGIME_META: Record<Regime, { label: string; color: string; thesis: string }> = {
    riskOff: {
      label: "Risk-OFF · уход из риска",
      color: "var(--down)",
      thesis:
        "Крупный капитал выходит из акций в облигации и валюту — защитная ротация.",
    },
    riskOn: {
      label: "Risk-ON · аппетит к риску",
      color: "var(--up)",
      thesis: "Деньги перетекают в акции из облигаций — растёт аппетит к риску.",
    },
    neutral: {
      label: "Нейтрально · ротация внутри классов",
      color: "#e0a23a",
      thesis:
        "Явного межклассового перетока нет — деньги перекладываются внутри классов.",
    },
  };

  const DECISIONS: Record<Regime, string[]> = {
    riskOff: [
      "Снижать долю акций, фиксировать прибыль в перекупленных секторах",
      "Наращивать ОФЗ — основной приток идёт в облигации",
      "Держать валютный хедж: USD/RUB и CNY/RUB притягивают капитал",
      "Сокращать плечо во фьючерсах на индекс",
    ],
    riskOn: [
      "Наращивать акции — деньги возвращаются в риск из облигаций",
      "Фокус на секторах-лидерах RRG (IT, Финансы)",
      "Сокращать защитные ОФЗ-позиции",
      "Допустимо умеренное плечо во фьючерсах",
    ],
    neutral: [
      "Без явного межклассового сигнала — работать внутри классов",
      "Парные идеи: лонг лидеров / шорт аутсайдеров RRG",
      "Держать сбалансированную аллокацию",
      "Ждать подтверждения по ширине рынка и CVD",
    ],
  };

  const RISKS = [
    "Резкий разворот RUB обнулит часть FX-притока",
    "Навес ОФЗ-размещений Минфина давит на облигации",
    "Тонкий рынок вечерней сессии усиливает шум потоков",
  ];

  const meta = $derived(REGIME_META[signal.regime]);
  const decisions = $derived(DECISIONS[signal.regime]);
  // Максимум по модулю — для нормировки диверг-баров.
  const maxAbs = $derived(
    Math.max(1, ...signal.classFlows.map((c) => Math.abs(c.netFlow))),
  );

  function flowStr(v: number): string {
    return `${v >= 0 ? "+" : "−"}₽${Math.abs(v)} млрд`;
  }
</script>

<div class="summary">
  <div class="regime" style="border-color:{meta.color}">
    <div class="regime-head">
      <span class="regime-cap">Режим рынка</span>
      <span class="regime-label" style="color:{meta.color}">{meta.label}</span>
    </div>
    <p class="thesis">{meta.thesis}</p>
    <div class="conviction">
      <span class="conv-cap">Уверенность сигнала</span>
      <div class="conv-bar">
        <span class="conv-fill" style="width:{signal.conviction}%;background:{meta.color}"></span>
      </div>
      <span class="conv-val" style="color:{meta.color}">{signal.conviction}</span>
    </div>
  </div>

  <div class="block">
    <div class="block-head">Карта больших денег · нетто-поток по классам</div>
    <div class="flows">
      {#each signal.classFlows as c (c.assetClass)}
        <div class="flow-row">
          <span class="flow-name">
            <span class="dot" style="background:{assetColor(c.assetClass)}"></span>
            {assetLabel(c.assetClass)}
          </span>
          <span class="flow-track">
            <span class="flow-neg">
              {#if c.netFlow < 0}
                <span
                  class="bar"
                  style="width:{(Math.abs(c.netFlow) / maxAbs) * 100}%;background:var(--down)"
                ></span>
              {/if}
            </span>
            <span class="flow-pos">
              {#if c.netFlow > 0}
                <span
                  class="bar"
                  style="width:{(Math.abs(c.netFlow) / maxAbs) * 100}%;background:var(--up)"
                ></span>
              {/if}
            </span>
          </span>
          <span class="flow-val" class:up={c.netFlow >= 0} class:down={c.netFlow < 0}>
            {flowStr(c.netFlow)}
          </span>
        </div>
      {/each}
    </div>
  </div>

  <div class="two-col">
    <div class="block">
      <div class="block-head">Что это значит · решения</div>
      <ul class="list">
        {#each decisions as d (d)}
          <li><span class="arrow" style="color:{meta.color}">→</span>{d}</li>
        {/each}
      </ul>
    </div>
    <div class="block">
      <div class="block-head">Риски · на что смотреть</div>
      <ul class="list">
        {#each RISKS as r (r)}
          <li><span class="warn">⚠</span>{r}</li>
        {/each}
      </ul>
    </div>
  </div>
</div>

<style>
  .summary {
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 12px;
  }
  .regime {
    border: 1px solid;
    border-radius: 8px;
    padding: 12px 14px;
    background: var(--bg-elev);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .regime-head {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .regime-cap {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--text-dim);
  }
  .regime-label {
    font-size: 17px;
    font-weight: 700;
  }
  .thesis {
    margin: 0;
    color: var(--text);
    line-height: 1.4;
  }
  .conviction {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .conv-cap {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--text-dim);
    white-space: nowrap;
  }
  .conv-bar {
    flex: 1;
    height: 7px;
    border-radius: 4px;
    background: var(--border);
    overflow: hidden;
  }
  .conv-fill {
    display: block;
    height: 100%;
    border-radius: 4px;
  }
  .conv-val {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    font-size: 15px;
  }
  .block {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-panel);
    overflow: hidden;
  }
  .block-head {
    padding: 7px 11px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-bottom: 1px solid var(--border);
  }
  .flows {
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .flow-row {
    display: grid;
    grid-template-columns: 120px 1fr 1fr 96px;
    align-items: center;
    gap: 6px;
  }
  .flow-name {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-dim);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
  }
  .flow-track {
    grid-column: 2 / 4;
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .flow-neg {
    display: flex;
    justify-content: flex-end;
    height: 12px;
  }
  .flow-pos {
    display: flex;
    justify-content: flex-start;
    height: 12px;
    border-left: 1px solid var(--border);
  }
  .bar {
    height: 100%;
    border-radius: 2px;
  }
  .flow-val {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .list {
    margin: 0;
    padding: 9px 13px;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .list li {
    display: flex;
    gap: 9px;
    align-items: flex-start;
    line-height: 1.35;
    color: var(--text);
  }
  .arrow {
    flex: none;
    font-weight: 700;
  }
  .warn {
    color: #e0a23a;
    flex: none;
  }
  .up {
    color: var(--up);
  }
  .down {
    color: var(--down);
  }
  @media (max-width: 720px) {
    .two-col {
      grid-template-columns: 1fr;
    }
  }
</style>
