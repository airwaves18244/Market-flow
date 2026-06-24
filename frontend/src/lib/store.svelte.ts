// Реактивное хранилище дашборда (Svelte 5 runes в `.svelte.ts`).
//
// Панели читают данные отсюда напрямую, поэтому докуемый layout (dockview)
// не обязан прокидывать пропсы в каждую панель — он отвечает только за
// расположение, а данные текут через общий реактивный стейт.

import { breadth as fetchBreadth, equityDashboard, flowSeries, rrg as fetchRrg } from "./ipc";
import type { Breadth, EquityDashboard, FlowPoint, RrgPoint } from "./types";

const DAY = 86_400;

class DashboardStore {
  fromTs = $state(0);
  toTs = $state(0);
  equity = $state<EquityDashboard | null>(null);
  flow = $state<FlowPoint[]>([]);
  breadth = $state<Breadth | null>(null);
  rrg = $state<RrgPoint[]>([]);
  focus = $state("SBER@MISX");
  error = $state<string | null>(null);
  loading = $state(false);

  /** Загрузить снимок за последние 30 дней. Идемпотентно — можно дёргать повторно. */
  async load(): Promise<void> {
    this.loading = true;
    this.error = null;
    const to = Math.floor(Date.now() / 1000);
    const from = to - 30 * DAY;
    this.fromTs = from;
    this.toTs = to;
    try {
      const [equity, breadth, rrg] = await Promise.all([
        equityDashboard(from, to),
        fetchBreadth(from, to),
        fetchRrg(from, to),
      ]);
      this.equity = equity;
      this.breadth = breadth;
      this.rrg = rrg;
      this.focus = equity.top_movers[0]?.symbol ?? this.focus;
      this.flow = await flowSeries(this.focus, from, to);
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    } finally {
      this.loading = false;
    }
  }
}

export const store = new DashboardStore();
