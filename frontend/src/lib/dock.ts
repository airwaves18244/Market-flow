// Интеграция dockview-core (vanilla) со Svelte 5: каждая докуемая панель —
// это Svelte-компонент, смонтированный в элемент панели. Данные компоненты
// берут из общего стора (`store.svelte.ts`), поэтому параметры панелей пусты.

import { mount, unmount, type Component } from "svelte";
import {
  createDockview,
  themeDark,
  type DockviewApi,
  type GroupPanelPartInitParameters,
  type IContentRenderer,
} from "dockview-core";

import SectorPanel from "./panels/SectorPanel.svelte";
import MoversPanel from "./panels/MoversPanel.svelte";
import FlowPanel from "./panels/FlowPanel.svelte";
import HeatmapPanel from "./panels/HeatmapPanel.svelte";
import BreadthPanel from "./panels/BreadthPanel.svelte";
import RrgPanel from "./panels/RrgPanel.svelte";

const registry: Record<string, Component> = {
  sectors: SectorPanel,
  movers: MoversPanel,
  flow: FlowPanel,
  heatmap: HeatmapPanel,
  breadth: BreadthPanel,
  rrg: RrgPanel,
};

class SvelteRenderer implements IContentRenderer {
  readonly element: HTMLElement;
  #instance: Record<string, unknown> | undefined;

  constructor(private readonly component: Component) {
    this.element = document.createElement("div");
    this.element.className = "dock-content";
  }

  init(_parameters: GroupPanelPartInitParameters): void {
    this.#instance = mount(this.component, { target: this.element });
  }

  dispose(): void {
    if (this.#instance) {
      void unmount(this.#instance);
      this.#instance = undefined;
    }
  }
}

/** Построить докуемый layout представления «Акции / секторы». */
export function buildLayout(node: HTMLElement): DockviewApi {
  const api = createDockview(node, {
    theme: themeDark,
    createComponent: (options) => new SvelteRenderer(registry[options.name] ?? SectorPanel),
  });

  api.addPanel({ id: "sectors", component: "sectors", title: "Секторы" });
  api.addPanel({
    id: "movers",
    component: "movers",
    title: "Топ-движения",
    position: { referencePanel: "sectors", direction: "right" },
  });
  api.addPanel({
    id: "heatmap",
    component: "heatmap",
    title: "Хитмэп секторов",
    position: { referencePanel: "sectors", direction: "below" },
  });
  api.addPanel({
    id: "flow",
    component: "flow",
    title: "Нетто-поток",
    position: { referencePanel: "heatmap", direction: "right" },
  });
  api.addPanel({
    id: "breadth",
    component: "breadth",
    title: "Ширина рынка",
    position: { referencePanel: "movers", direction: "below" },
  });
  api.addPanel({
    id: "rrg",
    component: "rrg",
    title: "RRG",
    position: { referencePanel: "breadth", direction: "below" },
  });

  return api;
}
