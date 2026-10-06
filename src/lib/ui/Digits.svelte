<script lang="ts">
  // A clock whose digits roll like an odometer when they change. Each
  // character sits in a fixed-width slot, so the numerals never jostle;
  // slots that appear or disappear (0:59 -> 1:00:00) open and close smoothly.
  import { cubicOut } from "svelte/easing";

  let {
    text,
    dir = "down",
    groups = false,
    onwheelgroup,
  }: {
    text: string;
    /** "down": new digits drop in from above (counting down); "up": rise from below. */
    dir?: "down" | "up";
    /** Mark the hour / minute / second groups for wheel input. */
    groups?: boolean;
    onwheelgroup?: (unit: "h" | "m" | "s", delta: number) => void;
  } = $props();

  // Align from the right so a new leading group grows on the left.
  const chars = $derived([...text].map((ch, i, all) => ({ ch, key: all.length - i })));

  function unitAt(index: number): "h" | "m" | "s" {
    const colons = [...text].filter((c) => c === ":").length;
    const before = [...text].slice(0, index).filter((c) => c === ":").length;
    if (colons === 2) return (["h", "m", "s"] as const)[before];
    return (["m", "s"] as const)[before];
  }

  function roll(_node: Element, { from }: { from: number }) {
    return {
      duration: 420,
      easing: cubicOut,
      css: (t: number, u: number) => `transform: translateY(${from * u * 62}%); opacity: ${t}; filter: blur(${u * 3}px)`,
    };
  }

  function grow(node: Element) {
    const w = (node as HTMLElement).offsetWidth;
    return {
      duration: 380,
      easing: cubicOut,
      css: (t: number) => `width: ${t * w}px; opacity: ${t}`,
    };
  }

  function wheel(e: WheelEvent, index: number) {
    if (!onwheelgroup) return;
    e.preventDefault();
    e.stopPropagation();
    onwheelgroup(unitAt(index), e.deltaY < 0 ? 1 : -1);
  }
</script>

<span class="digits" aria-hidden="true">
  {#each chars as c, i (c.key)}
    <span
      class="slot"
      class:sep={c.ch === ":"}
      class:group={groups && c.ch !== ":"}
      transition:grow
      onwheel={(e) => wheel(e, i)}
    >
      {#key c.ch}
        <span class="ch" in:roll={{ from: dir === "down" ? -1 : 1 }} out:roll={{ from: dir === "down" ? 1 : -1 }}>{c.ch}</span>
      {/key}
    </span>
  {/each}
</span>

<style>
  .digits {
    display: inline-flex;
    align-items: baseline;
    font-family: var(--font-num);
    font-variant-numeric: tabular-nums;
    line-height: 1;
    white-space: nowrap;
  }
  .slot {
    display: inline-grid;
    width: 0.6em;
    height: 1.12em;
    overflow: hidden;
    place-items: center;
    /* Fade the rolling digits at the slot's edges. */
    mask: linear-gradient(transparent, #000 18%, #000 82%, transparent);
  }
  .slot.sep {
    width: 0.3em;
    mask: none;
    transform: translateY(-0.06em);
    opacity: 0.55;
  }
  .ch {
    grid-area: 1 / 1;
  }
  .group {
    border-radius: 0.12em;
  }
</style>
