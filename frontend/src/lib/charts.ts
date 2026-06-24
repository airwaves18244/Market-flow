// Svelte-экшен для рендера ECharts в элемент и автоподгонки размера.
//
//   <div use:chart={option}></div>
//
// При смене `option` график обновляется, при удалении узла — освобождается.

import * as echarts from "echarts";
import type { EChartsOption } from "echarts";

export function chart(node: HTMLElement, option: EChartsOption) {
  const instance = echarts.init(node, "dark");
  instance.setOption(option);

  const observer = new ResizeObserver(() => instance.resize());
  observer.observe(node);

  return {
    update(next: EChartsOption) {
      instance.setOption(next, true);
    },
    destroy() {
      observer.disconnect();
      instance.dispose();
    },
  };
}
