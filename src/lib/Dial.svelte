<script lang="ts">
  // The ring. While setting, it is a dial: drag anywhere around it (one lap
  // is one hour) or click the ring to jump there. While running, the arc
  // unwinds as time passes, with a glowing head.
  import { untrack, type Snippet } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { Tween } from "svelte/motion";
  import { DIAL } from "./geometry";

  let {
    value,
    wrap = false,
    interactive = false,
    paused = false,
    warning = false,
    pulse = 0,
    onturn,
    onstep,
    children,
  }: {
    /** Ring fill. With `wrap`, in turns: 1.25 is one full lap plus a quarter. */
    value: number;
    wrap?: boolean;
    interactive?: boolean;
    paused?: boolean;
    warning?: boolean;
    /** Changes once a second during the warning, to beat the ring. */
    pulse?: number;
    /** Minutes turned (may be fractional; the parent snaps). */
    onturn?: (minutes: number, jumpTo: number | null) => void;
    onstep?: (minutes: number) => void;
    children?: Snippet;
  } = $props();

  const C = DIAL / 2;
  const R = 112;
  const STROKE = 12;
  const TICKS = Array.from({ length: 60 }, (_, i) => i);

  let el: HTMLDivElement;
  let dragging = $state(false);
  let last = 0;

  // Jumps (a chip, Add Time, Start) glide; dragging and the running
  // countdown follow exactly.
  const shown = new Tween(untrack(() => value), { duration: 560, easing: cubicOut });
  $effect(() => {
    const v = value;
    const jump = Math.abs(v - shown.current) > 0.004;
    shown.set(v, { duration: dragging || !jump ? 0 : 560 });
  });

  const fraction = $derived.by(() => {
    const v = shown.current;
    if (!wrap) return Math.max(0, Math.min(1, v));
    if (v <= 1e-6) return 0;
    const f = v - Math.floor(v);
    return f < 1e-6 ? 1 : f;
  });
  const laps = $derived(wrap ? Math.max(0, Math.ceil(shown.current - 1e-6) - 1) : 0);
  const deg = $derived(fraction * 360);

  function angleAt(e: PointerEvent): { a: number; d: number } {
    const r = el.getBoundingClientRect();
    const x = e.clientX - (r.left + r.width / 2);
    const y = e.clientY - (r.top + r.height / 2);
    let a = Math.atan2(x, -y);
    if (a < 0) a += Math.PI * 2;
    return { a, d: Math.hypot(x, y) };
  }

  function down(e: PointerEvent) {
    if (!interactive || e.button !== 0) return;
    // Controls inside the dial (the In / At switch) handle themselves.
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    e.stopPropagation();
    const { a, d } = angleAt(e);
    el.setPointerCapture(e.pointerId);
    dragging = true;
    last = a;
    // A press on the ring itself jumps the hand there.
    if (Math.abs(d - R) < 26) onturn?.(0, (a / (Math.PI * 2)) * 60);
  }

  function move(e: PointerEvent) {
    if (!dragging) return;
    const { a } = angleAt(e);
    let delta = a - last;
    if (delta > Math.PI) delta -= Math.PI * 2;
    if (delta < -Math.PI) delta += Math.PI * 2;
    last = a;
    onturn?.((delta / (Math.PI * 2)) * 60, null);
  }

  function up(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    el.releasePointerCapture(e.pointerId);
  }

  function wheel(e: WheelEvent) {
    if (!interactive) return;
    e.preventDefault();
    onstep?.((e.deltaY < 0 ? 1 : -1) * (e.shiftKey ? 5 : 1));
  }
</script>

<div
  class="dial"
  class:interactive
  class:dragging
  class:paused
  class:warning
  bind:this={el}
  style:--deg="{deg}deg"
  style:--size="{DIAL}px"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onwheel={wheel}
  role={interactive ? "slider" : "img"}
  aria-label="Timer Dial"
  aria-valuenow={Math.round(fraction * 60)}
  tabindex="-1"
>
  <svg class="ticks" viewBox="0 0 {DIAL} {DIAL}" aria-hidden="true">
    {#each TICKS as i (i)}
      {@const long = i % 5 === 0}
      {@const lit = i / 60 < fraction - 0.0001 || laps > 0}
      <line
        x1={C}
        y1={long ? 4 : 6}
        x2={C}
        y2={long ? 11 : 9.5}
        class:long
        class:lit
        transform="rotate({i * 6} {C} {C})"
      />
    {/each}
  </svg>

  <div class="track" style:--r="{R}px" style:--w="{STROKE}px"></div>
  {#if laps > 0}
    <div class="lap" style:--r="{R}px" style:--w="{STROKE}px"></div>
  {/if}
  <div class="sheen" style:--r="{R}px" style:--w="{STROKE}px"></div>
  {#key pulse}
    <div class="arc glow" class:beat={warning} style:--r="{R}px" style:--w="{STROKE}px"></div>
  {/key}
  <div class="arc" style:--r="{R}px" style:--w="{STROKE}px"></div>
  {#if deg > 0.5}
    <div class="cap" style:--r="{R}px" style:--w="{STROKE}px"></div>
  {/if}
  <div class="hand" style:--r="{R}px" style:rotate="{deg}deg">
    <div class="knob" class:big={interactive}></div>
  </div>

  <div class="center">
    {@render children?.()}
  </div>
</div>

<style>
  .dial {
    position: relative;
    width: var(--size);
    height: var(--size);
    border-radius: 50%;
    touch-action: none;
  }
  .dial.interactive {
    cursor: grab;
  }
  .dial.dragging {
    cursor: grabbing;
  }
  .ticks {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .ticks line {
    stroke: rgb(255 255 255 / 0.13);
    stroke-width: 1.4;
    stroke-linecap: round;
    transition: stroke 0.35s ease;
  }
  .ticks line.long {
    stroke: rgb(255 255 255 / 0.24);
    stroke-width: 2;
  }
  .ticks line.lit {
    stroke: color-mix(in oklab, var(--accent) 70%, transparent);
  }
  .ticks line.lit.long {
    stroke: var(--accent);
  }

  /* Rings are discs masked to an annulus of radius --r and width --w. */
  .track,
  .lap,
  .sheen,
  .arc {
    position: absolute;
    left: 50%;
    top: 50%;
    width: calc(var(--r) * 2 + var(--w));
    height: calc(var(--r) * 2 + var(--w));
    translate: -50% -50%;
    border-radius: 50%;
    mask: radial-gradient(farthest-side, transparent calc(100% - var(--w)), #000 calc(100% - var(--w) + 0.6px), #000 calc(100% - 0.6px), transparent);
  }
  .track {
    background: rgb(255 255 255 / 0.06);
  }
  .lap {
    background: color-mix(in oklab, var(--accent) 26%, transparent);
  }
  .arc {
    background: conic-gradient(from 0deg, var(--accent-2), var(--accent) var(--deg), transparent var(--deg));
    transition: filter 0.4s;
  }
  .paused .arc {
    filter: saturate(0.15) brightness(0.8);
  }
  .arc.glow {
    filter: blur(9px);
    opacity: 0.75;
    mask: none;
    background: conic-gradient(from 0deg, transparent 0deg, var(--accent-2) 2deg, var(--accent) var(--deg), transparent var(--deg));
    -webkit-mask: radial-gradient(farthest-side, transparent calc(100% - var(--w) - 14px), #000 calc(100% - var(--w) - 4px), #000 calc(100% + 4px));
  }
  .paused .arc.glow {
    opacity: 0.15;
  }
  .arc.glow.beat {
    animation: beat 1s var(--ease-out);
  }
  @keyframes beat {
    0% {
      opacity: 1;
      filter: blur(14px) brightness(1.4);
    }
    100% {
      opacity: 0.6;
      filter: blur(9px);
    }
  }
  /* A soft light that travels around the track while the dial is idle. */
  .sheen {
    background: conic-gradient(from 0deg, transparent 0deg, rgb(255 255 255 / 0.1) 30deg, transparent 70deg);
    animation: spin 7s linear infinite;
  }
  .paused .sheen {
    animation-play-state: paused;
    opacity: 0;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }
  /* Round start cap at 12 o'clock. */
  .cap {
    position: absolute;
    left: 50%;
    top: calc(50% - var(--r));
    width: var(--w);
    height: var(--w);
    translate: -50% -50%;
    border-radius: 50%;
    background: var(--accent-2);
  }
  .paused .cap {
    filter: saturate(0.15) brightness(0.8);
  }
  .hand {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 0;
    height: 0;
  }
  .knob {
    position: absolute;
    left: 0;
    top: calc(var(--r) * -1);
    width: 14px;
    height: 14px;
    translate: -50% -50%;
    border-radius: 50%;
    background: #fff;
    box-shadow:
      0 0 0 3px var(--accent),
      0 0 18px 4px color-mix(in oklab, var(--accent) 70%, transparent);
    transition:
      width 0.3s var(--spring),
      height 0.3s var(--spring),
      box-shadow 0.3s;
  }
  .knob.big {
    width: 24px;
    height: 24px;
    box-shadow:
      0 0 0 5px color-mix(in oklab, var(--accent) 90%, white),
      0 4px 14px rgb(0 0 0 / 0.5),
      0 0 22px 6px color-mix(in oklab, var(--accent) 60%, transparent);
  }
  .interactive:hover .knob.big,
  .dragging .knob.big {
    width: 28px;
    height: 28px;
  }
  .paused .knob {
    background: #d8d6e0;
    box-shadow:
      0 0 0 3px #77748a,
      0 0 0 0 transparent;
  }
  .center {
    position: absolute;
    inset: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
  }
</style>
