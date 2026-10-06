<script lang="ts">
  // The countdown as a floating pill. It grows on hover to show controls,
  // grows further in the last minute, and shows notices.
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { PillSize } from "./geometry";
  import Digits from "./ui/Digits.svelte";
  import Icon from "./ui/Icon.svelte";
  import { tip } from "./ui/tip";
  import { actionInfo, app } from "./state.svelte";
  import { clock, timeOfDay } from "./time";

  let {
    size,
    onpause,
    onresume,
    oncancel,
    onextend,
    onexpand,
    onhide,
  }: {
    size: PillSize;
    onpause: () => void;
    onresume: () => void;
    oncancel: () => void;
    onextend: (sec: number) => void;
    onexpand: () => void;
    onhide: () => void;
  } = $props();

  const info = $derived(actionInfo(app.action));
  const paused = $derived(app.phase === "paused");
  const frac = $derived(Math.max(0, Math.min(1, app.fraction)));

  function ring(px: number, stroke: number) {
    const r = (px - stroke) / 2;
    const c = 2 * Math.PI * r;
    return { r, c, half: px / 2 };
  }
  const small = ring(26, 3.4);
  const mid = ring(34, 3.6);
  const large = ring(78, 5);

  const stagger = (i: number) => ({ duration: 260, delay: 70 + i * 35, start: 0.6, easing: cubicOut });
</script>

{#snippet progress(g: { r: number; c: number; half: number }, stroke: number, inner?: string)}
  <svg class="ring" class:paused width={g.half * 2} height={g.half * 2} viewBox="0 0 {g.half * 2} {g.half * 2}" aria-hidden="true">
    <defs>
      <linearGradient id="pill-arc-{g.half}" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" style="stop-color: var(--accent-2)" />
        <stop offset="1" style="stop-color: var(--accent)" />
      </linearGradient>
    </defs>
    <circle cx={g.half} cy={g.half} r={g.r} class="track" stroke-width={stroke} />
    <circle
      cx={g.half}
      cy={g.half}
      r={g.r}
      class="arc"
      stroke="url(#pill-arc-{g.half})"
      stroke-width={stroke}
      stroke-dasharray={g.c}
      stroke-dashoffset={g.c * (1 - frac)}
      transform="rotate(-90 {g.half} {g.half})"
    />
    {#if inner}
      <text x="50%" y="50%" class="inner num" dominant-baseline="central" text-anchor="middle">{inner}</text>
    {/if}
  </svg>
{/snippet}

<div class="pill {size}" class:paused class:warn={size === "warning"}>
  {#if size === "compact"}
    <div class="row compact" in:fade={{ duration: 220, delay: 90 }}>
      {@render progress(small, 3.4)}
      <span class="time num"><Digits text={clock(app.seconds)} /></span>
      <span class="act"><Icon name={paused ? "pause" : info.icon} size={15} stroke={2.2} /></span>
    </div>
  {:else if size === "hover"}
    <div class="row hover" in:fade={{ duration: 200, delay: 60 }}>
      {@render progress(mid, 3.6)}
      <div class="col">
        <span class="time num big"><Digits text={clock(app.seconds)} /></span>
        <span class="sub">{paused ? "Paused" : app.endsAt ? `At ${timeOfDay(app.endsAt)}` : info.label}</span>
      </div>
      <div class="btns">
        {#if paused}
          <button class="pb" onclick={onresume} use:tip={{ text: "Resume", side: "bottom" }} in:scale={stagger(0)}><Icon name="play" size={15} fill /></button>
        {:else}
          <button class="pb" onclick={onpause} use:tip={{ text: "Pause", side: "bottom" }} in:scale={stagger(0)}><Icon name="pause" size={16} stroke={2.4} /></button>
        {/if}
        <button class="pb" onclick={() => onextend(300)} use:tip={{ text: "Add 5 Minutes", side: "bottom" }} in:scale={stagger(1)}><span class="plus num">+5</span></button>
        <button class="pb" onclick={onexpand} use:tip={{ text: "Open Full View", side: "bottom" }} in:scale={stagger(2)}><Icon name="expand" size={16} stroke={2} /></button>
        <button class="pb" onclick={onhide} use:tip={{ text: "Hide to Tray", side: "bottom" }} in:scale={stagger(3)}><Icon name="tray" size={16} stroke={2} /></button>
        <button class="pb x" onclick={oncancel} use:tip={{ text: "Cancel Timer", side: "bottom" }} in:scale={stagger(4)}><Icon name="x" size={15} stroke={2.4} /></button>
      </div>
    </div>
  {:else if size === "warning"}
    <div class="row warning" in:fade={{ duration: 240, delay: 120 }}>
      <div class="big-ring">
        {#key app.seconds}<span class="halo"></span>{/key}
        {@render progress(large, 5, String(app.seconds))}
      </div>
      <div class="col grow">
        <span class="title">{info.doing} in {app.seconds} {app.seconds === 1 ? "Second" : "Seconds"}</span>
        <span class="sub">{app.action === "sleep" ? "The computer will go to sleep." : "Open apps will be closed without saving."}</span>
        <div class="wbtns">
          <button class="wb" onclick={() => onextend(300)} in:scale={stagger(1)}><Icon name="plus" size={13} stroke={2.6} />5 Min</button>
          <button class="wb" onclick={() => onextend(900)} in:scale={stagger(2)}><Icon name="plus" size={13} stroke={2.6} />15 Min</button>
          <button class="wb stop" onclick={oncancel} in:scale={stagger(3)}>Cancel</button>
        </div>
      </div>
    </div>
  {:else if size === "firing"}
    <div class="row firing" in:fade={{ duration: 220, delay: 90 }}>
      <span class="spin"></span>
      <span class="title">{info.doing}…</span>
    </div>
  {:else if size === "notice" && app.toast}
    <div class="row notice" in:fade={{ duration: 220, delay: 90 }}>
      <span class="ndot" class:err={app.toast.error}></span>
      <span class="ntext">{app.toast.text}</span>
    </div>
  {/if}
</div>

<style>
  .pill {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: inherit;
  }
  .row {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
  }
  /* Sizes match compactWidth() in geometry.ts. */
  .compact {
    padding: 0 14px 0 9px;
    gap: 9px;
  }
  .hover {
    padding: 0 10px 0 12px;
    gap: 11px;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .grow {
    flex: 1;
  }
  .time {
    font-size: 19px;
    font-weight: 400;
    color: var(--text);
  }
  .time.big {
    font-size: 20px;
  }
  .compact .time {
    flex: 1;
    min-width: 0;
  }
  .paused .time {
    animation: blink 1.6s ease-in-out infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .sub {
    font-size: 11px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .act {
    color: var(--accent);
    filter: drop-shadow(0 0 6px color-mix(in oklab, var(--accent) 80%, transparent));
  }
  .ring {
    flex: none;
    overflow: visible;
  }
  .ring .track {
    fill: none;
    stroke: rgb(255 255 255 / 0.1);
  }
  .ring .arc {
    fill: none;
    stroke-linecap: round;
    filter: drop-shadow(0 0 4px color-mix(in oklab, var(--accent) 80%, transparent));
    transition: stroke-dashoffset 0.25s linear;
  }
  .ring.paused .arc {
    stroke: #8a879b;
    filter: none;
  }
  .ring .inner {
    fill: var(--text);
    font-size: 28px;
    font-weight: 400;
  }
  .btns {
    margin-left: auto;
    display: flex;
    gap: 2px;
  }
  .pb {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--text-2);
    transition:
      background-color 0.15s,
      color 0.15s,
      scale 0.2s var(--spring);
  }
  .pb:hover {
    background: var(--hover);
    color: var(--text);
  }
  .pb:active {
    scale: 0.88;
  }
  .pb.x:hover {
    background: color-mix(in oklab, var(--danger) 25%, transparent);
    color: #ffb3b6;
  }
  .plus {
    font-size: 13px;
    font-weight: 600;
  }

  .warning {
    padding: 0 16px 0 15px;
    gap: 15px;
  }
  .big-ring {
    position: relative;
    flex: none;
    display: grid;
    place-items: center;
  }
  .halo {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    box-shadow: 0 0 0 0 var(--accent);
    animation: halo 1s var(--ease-out) both;
  }
  @keyframes halo {
    from {
      box-shadow: 0 0 0 0 color-mix(in oklab, var(--accent) 70%, transparent);
    }
    to {
      box-shadow: 0 0 0 16px transparent;
    }
  }
  .title {
    font-size: 14.5px;
    font-weight: 700;
    white-space: nowrap;
  }
  .wbtns {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .wb {
    height: 28px;
    padding: 0 10px 0 8px;
    border-radius: 99px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 12px;
    font-weight: 700;
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--line-2);
    transition:
      background-color 0.15s,
      scale 0.2s var(--spring);
  }
  .wb:hover {
    background: var(--hover);
  }
  .wb:active {
    scale: 0.93;
  }
  .wb.stop {
    margin-left: auto;
    padding: 0 14px;
    color: #1a0d10;
    background: linear-gradient(120deg, #ff8a8f, var(--danger));
    box-shadow: 0 4px 16px -2px color-mix(in oklab, var(--danger) 70%, transparent);
  }
  .wb.stop:hover {
    filter: brightness(1.08);
  }

  .firing {
    justify-content: center;
    gap: 12px;
  }
  .spin {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2.5px solid var(--line-2);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }

  .notice {
    padding: 0 20px;
    gap: 12px;
  }
  .ndot {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 10px var(--accent);
  }
  .ndot.err {
    background: var(--danger);
    box-shadow: 0 0 10px var(--danger);
  }
  .ntext {
    font-size: 12.5px;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
