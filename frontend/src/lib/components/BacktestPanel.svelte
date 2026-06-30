<script lang="ts">
  // Вкладка «Бэктест» — ПРОТОТИП без бэкенда. Результат симулируется в UI
  // детерминированным сид-генератором. Реальный движок (event-driven по
  // сохранённым барам DuckDB) описан в ROADMAP.md.

  const PRESETS = [
    "Ротация в ОФЗ при Risk-OFF",
    "Секторный моментум · RRG-лидеры",
    "Пробой на аномальном объёме",
  ];

  let preset = $state(0);
  let seed = $state(3);

  function run() {
    seed += 1;
  }

  // Детерминированный ГПСЧ (LCG) — один и тот же сид даёт один и тот же прогон.
  function makeRng(s: number): () => number {
    let r = s * 1000 + 7;
    return () => {
      r = (r * 9301 + 49297) % 233280;
      return r / 233280;
    };
  }

  type Curve = { strat: number[]; bench: number[] };

  function genEquity(s: number): Curve {
    const rnd = makeRng(s);
    const n = 180;
    let st = 100;
    let bn = 100;
    const strat: number[] = [];
    const bench: number[] = [];
    for (let i = 0; i < n; i++) {
      st *= 1 + (rnd() - 0.45) * 0.02 + 0.0013;
      bn *= 1 + (rnd() - 0.48) * 0.018 + 0.0006;
      strat.push(st);
      bench.push(bn);
    }
    return { strat, bench };
  }

  const curve = $derived(genEquity(seed));

  const stats = $derived.by(() => {
    const st = curve.strat;
    const bn = curve.bench;
    const n = st.length;
    const cagr = (Math.pow(st[n - 1] / 100, 252 / n) - 1) * 100;
    const rets = st.slice(1).map((v, i) => v / st[i] - 1);
    const mean = rets.reduce((s, x) => s + x, 0) / rets.length;
    const sd = Math.sqrt(rets.reduce((s, x) => s + (x - mean) ** 2, 0) / rets.length);
    const sharpe = sd ? (mean / sd) * Math.sqrt(252) : 0;
    let peak = st[0];
    let mdd = 0;
    for (const v of st) {
      peak = Math.max(peak, v);
      mdd = Math.min(mdd, (v - peak) / peak);
    }
    const win = (rets.filter((x) => x > 0).length / rets.length) * 100;
    const benchCagr = (Math.pow(bn[n - 1] / 100, 252 / n) - 1) * 100;
    return [
      { k: "CAGR", v: `${cagr >= 0 ? "+" : ""}${cagr.toFixed(1)}%`, cls: cagr >= 0 ? "up" : "down" },
      { k: "Sharpe", v: sharpe.toFixed(2), cls: "" },
      { k: "Макс. DD", v: `${(mdd * 100).toFixed(1)}%`, cls: "down" },
      { k: "Винрейт", v: `${win.toFixed(0)}%`, cls: "" },
      { k: "Сделок", v: `${40 + seed * 3}`, cls: "dim" },
      { k: "Бенчмарк", v: `${benchCagr >= 0 ? "+" : ""}${benchCagr.toFixed(1)}%`, cls: "dim" },
    ];
  });

  // SVG кривой капитала (стратегия vs IMOEX).
  const W = 640;
  const H = 220;
  const path = $derived.by(() => {
    const all = [...curve.strat, ...curve.bench];
    const mn = Math.min(...all);
    const mx = Math.max(...all);
    const n = curve.strat.length;
    const X = (i: number) => (i / (n - 1)) * W;
    const Y = (v: number) => 8 + (1 - (v - mn) / (mx - mn || 1)) * (H - 16);
    const line = (a: number[]) =>
      a.map((v, i) => `${i ? "L" : "M"}${X(i).toFixed(1)} ${Y(v).toFixed(1)}`).join(" ");
    return { strat: line(curve.strat), bench: line(curve.bench) };
  });

  // Доходность по месяцам (детерминированно от сида).
  const MONTHS = ["Я", "Ф", "М", "А", "М", "И", "И", "А", "С", "О", "Н", "Д"];
  const monthly = $derived.by(() => {
    const rnd = makeRng(seed * 131 + 5);
    return [2025, 2026].map((yr) => ({
      yr,
      cells: MONTHS.map(() => {
        const v = (rnd() - 0.45) * 9;
        return { v, str: `${v >= 0 ? "+" : ""}${v.toFixed(1)}` };
      }),
    }));
  });

  function heatBg(v: number): string {
    const t = Math.min(Math.abs(v) / 8, 1);
    const base = [23, 27, 36];
    const target = v >= 0 ? [44, 156, 99] : [200, 66, 61];
    const c = base.map((b, i) => Math.round(b + (target[i] - b) * t));
    return `rgb(${c[0]},${c[1]},${c[2]})`;
  }
</script>

<div class="bt">
  <div class="side">
    <div class="side-head">
      <span>Стратегия</span>
      <span class="badge">ПРОТОТИП</span>
    </div>
    <div class="field">
      <span class="cap">Пресет</span>
      {#each PRESETS as p, i (p)}
        <button class="preset" class:active={i === preset} onclick={() => (preset = i)}>{p}</button>
      {/each}
    </div>
    <div class="field">
      <span class="cap">Параметры</span>
      <div class="param"><span>Капитал</span><b>₽1 000 000</b></div>
      <div class="param"><span>Сайзинг</span><b>риск 1% / сделка</b></div>
      <div class="param"><span>Комиссия</span><b>0.04%</b></div>
    </div>
    <button class="run" onclick={run}>▶ Запустить бэктест</button>
    <p class="note">
      Прототип: результат симулируется в UI. Реальный движок будет считать на
      сохранённых барах (DuckDB) — см. ROADMAP.md.
    </p>
  </div>

  <div class="main">
    <div class="stats">
      {#each stats as s (s.k)}
        <div class="stat">
          <span class="cap">{s.k}</span>
          <span class="stat-v {s.cls}">{s.v}</span>
        </div>
      {/each}
    </div>

    <div class="chart">
      <div class="chart-head">
        <span>Кривая капитала vs IMOEX</span>
        <span class="legend">
          <span class="leg strat">стратегия</span>
          <span class="leg bench">IMOEX</span>
        </span>
      </div>
      <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" class="curve">
        <path d={path.bench} class="p-bench" />
        <path d={path.strat} class="p-strat" />
      </svg>
    </div>

    <div class="chart">
      <div class="chart-head"><span>Доходность по месяцам, %</span></div>
      <div class="months">
        <div class="mrow head">
          <span></span>
          {#each MONTHS as m, i (i)}<span class="ml">{m}</span>{/each}
        </div>
        {#each monthly as y (y.yr)}
          <div class="mrow">
            <span class="yr">{y.yr}</span>
            {#each y.cells as c, i (i)}
              <span class="cell" style="background:{heatBg(c.v)}">{c.str}</span>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .bt {
    display: grid;
    grid-template-columns: 280px 1fr;
    gap: 10px;
    font-size: 12px;
  }
  .side,
  .main {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .side {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-panel);
    padding: 12px;
  }
  .side-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: 600;
  }
  .badge {
    font-size: 9px;
    color: #e0a23a;
    border: 1px solid rgba(224, 162, 58, 0.4);
    border-radius: 3px;
    padding: 1px 6px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .cap {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
  }
  .preset {
    text-align: left;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-dim);
    border-radius: 6px;
    padding: 8px 10px;
    cursor: pointer;
    font-size: 12px;
  }
  .preset.active {
    border-color: var(--accent);
    background: rgba(79, 156, 249, 0.12);
    color: var(--text);
  }
  .param {
    display: flex;
    justify-content: space-between;
    color: var(--text-dim);
  }
  .param b {
    color: var(--text);
    font-weight: 600;
  }
  .run {
    border: none;
    border-radius: 7px;
    padding: 11px;
    font-weight: 600;
    cursor: pointer;
    color: #0b0d12;
    background: var(--accent);
  }
  .note {
    margin: 0;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-dim);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 8px;
  }
  .stat {
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--bg-panel);
    padding: 9px 11px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .stat-v {
    font-size: 17px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .chart {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-panel);
    overflow: hidden;
  }
  .chart-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 7px 11px;
    border-bottom: 1px solid var(--border);
    font-weight: 600;
    color: var(--text-dim);
    font-size: 11px;
  }
  .legend {
    display: flex;
    gap: 12px;
  }
  .leg {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .leg::before {
    content: "";
    width: 14px;
    height: 2px;
  }
  .leg.strat::before {
    background: var(--accent);
  }
  .leg.bench::before {
    border-top: 2px dashed var(--text-dim);
  }
  .curve {
    display: block;
    width: 100%;
    height: 200px;
    padding: 6px;
  }
  .p-strat {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }
  .p-bench {
    fill: none;
    stroke: var(--text-dim);
    stroke-width: 1.5;
    stroke-dasharray: 4 3;
    vector-effect: non-scaling-stroke;
  }
  .months {
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .mrow {
    display: grid;
    grid-template-columns: 46px repeat(12, 1fr);
    gap: 3px;
    align-items: center;
  }
  .mrow.head .ml,
  .yr {
    font-size: 10px;
    color: var(--text-dim);
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .yr {
    text-align: left;
  }
  .cell {
    font-size: 10px;
    font-weight: 600;
    text-align: center;
    padding: 6px 1px;
    border-radius: 3px;
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.88);
  }
  .up {
    color: var(--up);
  }
  .down {
    color: var(--down);
  }
  .dim {
    color: var(--text-dim);
  }
  @media (max-width: 720px) {
    .bt {
      grid-template-columns: 1fr;
    }
    .stats {
      grid-template-columns: repeat(3, 1fr);
    }
  }
</style>
