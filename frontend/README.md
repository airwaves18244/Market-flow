# Frontend (Tauri webview)

Веб-фронт терминала. Каркас собран в Фазе 3.

## Стек
- **Vite + TypeScript + Svelte 5** — лёгкий рантайм, быстрый на потоковых обновлениях.
- **ECharts** — treemap, heatmap, sunburst, **Sankey**, stacked area, gauge.
- **TradingView Lightweight Charts** — свечи/цена/объём.
- (планируется) **TanStack Table** — плотные таблицы; **dockview** — докуемые панели.

## Что уже есть (Фаза 3)
- Тёмная тема, каркас докуемых панелей (CSS-grid; полноценный dockview — позже).
- Типизированный клиент IPC `src/lib/ipc.ts` — зеркало DTO из `crates/app`:
  `instruments`, `bars`, `turnover_series`, `sector_rollup`, `sector_map`.
- **Мок-режим**: вне Tauri (`src/lib/mock.ts`) UI работает в обычном браузере на
  демо-данных — можно разрабатывать и собирать без бэкенда.
- Панели: секторный **treemap** (ECharts), **свечной график** (Lightweight
  Charts), список инструментов.

## Связь с ядром
Данные приходят из Rust-ядра через Tauri:
- `invoke(...)` — запрос снимков и временных рядов (команды из `crates/app`);
- события Tauri — live-push котировок/сделок во фронт (Фаза 7).

Аргументы команд именуются camelCase — Tauri преобразует их в snake_case
параметры Rust.

## Команды
```bash
npm install
npm run dev      # http://localhost:5173 (мок-данные вне Tauri)
npm run build    # сборка в dist/ (её подхватывает Tauri)
npm run check    # svelte-check (типы)
```

## Запуск как десктоп (Tauri)
Требуется десктопное окружение (на Linux — webkit2gtk). Из корня репозитория:
```bash
cargo run -p app --features tauri
```
Tauri сам поднимет dev-сервер фронта (см. `crates/app/tauri.conf.json`).
