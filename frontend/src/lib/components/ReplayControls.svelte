<script lang="ts">
  import { onDestroy } from "svelte";
  import type { ReplayStateDto } from "../types";

  let {
    replay = null,
    onSeek,
  }: { replay: ReplayStateDto | null; onSeek: (played: number) => void } = $props();

  let playing = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;

  function stop() {
    playing = false;
    if (timer) {
      clearInterval(timer);
      timer = undefined;
    }
  }

  function tick() {
    const cur = replay;
    if (!cur || cur.atEnd) {
      stop();
      return;
    }
    onSeek(cur.pos + 1);
  }

  function togglePlay() {
    if (playing) {
      stop();
      return;
    }
    if (replay?.atEnd) onSeek(0); // в конце — начать заново
    playing = true;
    timer = setInterval(tick, 500);
  }

  function step(delta: number) {
    stop();
    const pos = (replay?.pos ?? 0) + delta;
    onSeek(Math.max(0, pos));
  }

  function reset() {
    stop();
    onSeek(0);
  }

  function fmtTs(ts: number | null): string {
    if (ts == null) return "—";
    return new Date(ts * 1000).toISOString().slice(0, 10);
  }

  onDestroy(stop);
</script>

<div class="replay">
  <div class="buttons">
    <button onclick={reset} title="В начало" aria-label="В начало">⏮</button>
    <button onclick={() => step(-1)} title="Назад" aria-label="Шаг назад">◀</button>
    <button class="play" onclick={togglePlay} title={playing ? "Пауза" : "Воспроизвести"}>
      {playing ? "⏸" : "▶"}
    </button>
    <button onclick={() => step(1)} title="Вперёд" aria-label="Шаг вперёд">▶</button>
  </div>

  <div class="track">
    <div class="fill" style="width: {(replay?.progress ?? 0) * 100}%"></div>
  </div>

  <div class="meta">
    <span>Кадр {replay?.pos ?? 0} / {replay?.frames ?? 0}</span>
    <span>{fmtTs(replay?.currentTs ?? null)}</span>
    <span>{Math.round((replay?.progress ?? 0) * 100)}%</span>
  </div>
</div>

<style>
  .replay {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px;
  }
  .buttons {
    display: flex;
    gap: 6px;
    justify-content: center;
  }
  button {
    background: var(--bg-elev);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 10px;
    cursor: pointer;
    font-size: 13px;
  }
  button:hover {
    border-color: var(--accent);
  }
  button.play {
    color: var(--accent);
    font-weight: 700;
  }
  .track {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-elev);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s linear;
  }
  .meta {
    display: flex;
    justify-content: space-between;
    color: var(--text-dim);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
</style>
