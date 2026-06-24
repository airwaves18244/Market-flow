# Frontend (Tauri webview)

Веб-фронт терминала. Подключается в Фазе 3.

## Планируемый стек
- **Vite + TypeScript + Svelte** — лёгкий рантайм, быстрый на потоковых обновлениях.
- **ECharts** — treemap, heatmap, sunburst, **Sankey**, stacked area, gauge.
- **TradingView Lightweight Charts** — свечи/цена/объём.
- **TanStack Table** — плотные таблицы с сортировкой и виртуализацией.
- **dockview** — докуемые панели, мульти-монитор.

## Связь с ядром
Данные приходят из Rust-ядра через Tauri:
- `invoke(...)` — запрос снимков и временных рядов (команды из `crates/app`);
- события Tauri — live-push котировок/сделок во фронт.

## Инициализация (в Фазе 3)
```bash
npm create vite@latest . -- --template svelte-ts
npm i echarts lightweight-charts @tanstack/svelte-table dockview-core
```
