<script lang="ts">
  import type { InstrumentDto } from "../types";

  let {
    items = [],
    selected,
    onSelect,
  }: {
    items: InstrumentDto[];
    selected?: string;
    onSelect?: (symbol: string) => void;
  } = $props();
</script>

<table>
  <thead>
    <tr>
      <th>Тикер</th>
      <th>Название</th>
      <th>Сектор</th>
    </tr>
  </thead>
  <tbody>
    {#each items as it (it.symbol)}
      <tr
        class:selected={it.symbol === selected}
        onclick={() => onSelect?.(it.symbol)}
      >
        <td>{it.ticker}</td>
        <td>{it.name}</td>
        <td>{it.sector ?? "—"}</td>
      </tr>
    {/each}
  </tbody>
</table>
