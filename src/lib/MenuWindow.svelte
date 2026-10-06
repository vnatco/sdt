<script lang="ts">
  // The pop-up menu window. It receives items, measures itself, asks the
  // backend to place it on screen, and reports the pick.
  import { onMount, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, events, type MenuItem } from "./api";
  import Icon from "./ui/Icon.svelte";

  const MARGIN = 16;

  let items = $state<MenuItem[]>([]);
  let shown = $state(false);
  let closing = $state(false);
  let origin = $state("top left");
  let el: HTMLDivElement | undefined = $state();
  let generation = 0;

  async function open(next: MenuItem[]) {
    const gen = ++generation;
    shown = false;
    closing = false;
    items = next;
    await tick();
    if (!el || gen !== generation) return;
    // Layout is synchronous even while the window is hidden.
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    try {
      const p = await api.menuPlace(w, h);
      if (gen !== generation) return;
      origin = `${p.flipY ? "bottom" : "top"} ${p.flipX ? "right" : "left"}`;
      shown = true;
      requestAnimationFrame(() => (el?.querySelector<HTMLButtonElement>("button:not(:disabled)"))?.focus({ preventScroll: true }));
    } catch (e) {
      console.error("menu placement failed", e);
      await api.menuClose().catch(() => {});
    }
  }

  async function close(pick?: string) {
    if (closing || !shown) return;
    closing = true;
    // Let the fade play, then hide; the empty frame is what shows next time.
    await new Promise((r) => setTimeout(r, 110));
    shown = false;
    try {
      if (pick) await api.menuPick(pick);
      else await api.menuClose();
    } catch (e) {
      console.error("menu close failed", e);
    }
    closing = false;
  }

  function key(e: KeyboardEvent) {
    if (!shown) return;
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const btns = [...(el?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
      const i = btns.indexOf(document.activeElement as HTMLButtonElement);
      btns[(i + (e.key === "ArrowDown" ? 1 : -1) + btns.length) % btns.length]?.focus();
    }
  }

  onMount(() => {
    const unsubs = [
      events.menuShow((m) => {
        document.documentElement.dataset.action = m.action;
        open(m.items);
      }),
      getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (!focused) close();
      }),
    ];
    return () => unsubs.forEach((u) => u.then((f) => f()));
  });
</script>

<svelte:window onkeydown={key} oncontextmenu={(e) => e.preventDefault()} />

<!-- A click on the transparent shadow margin closes the menu. -->
<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="menu" class:shown class:closing bind:this={el} role="menu" style:margin="{MARGIN}px" style:transform-origin={origin}>
    {#each items as it, i (i)}
      {#if it.kind === "sep"}
        <div class="sep" role="separator"></div>
      {:else if it.kind === "label"}
        <div class="lbl">{it.label}</div>
      {:else}
        <button
          role={it.checked === undefined ? "menuitem" : "menuitemradio"}
          aria-checked={it.checked}
          class:danger={it.danger}
          disabled={it.disabled}
          style:--d="{Math.min(i, 14) * 12}ms"
          onclick={() => close(it.id)}
        >
          <span class="ic">
            {#if it.checked}
              <span class="check"><Icon name="check" size={14} stroke={2.6} /></span>
            {:else if it.icon}
              <Icon name={it.icon} size={15} stroke={2} fill />
            {/if}
          </span>
          <span class="t">{it.label}</span>
          {#if it.hint}<span class="hint">{it.hint}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .backdrop {
    position: fixed;
    inset: 0;
  }
  .menu {
    display: inline-block;
    min-width: 224px;
    padding: 6px;
    border-radius: 14px;
    background: linear-gradient(180deg, #24222c, #1c1a22);
    box-shadow:
      0 0 0 1px rgb(255 255 255 / 0.09),
      inset 0 1px 0 rgb(255 255 255 / 0.06),
      0 12px 32px rgb(0 0 0 / 0.55);
    font-size: 13px;
    opacity: 0;
  }
  .menu.shown {
    animation: pop 0.22s var(--ease-out) both;
  }
  .menu.closing {
    animation: out 0.11s ease-in both;
  }
  @keyframes pop {
    from {
      opacity: 0;
      scale: 0.92;
    }
    to {
      opacity: 1;
      scale: 1;
    }
  }
  @keyframes out {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
      scale: 0.97;
    }
  }
  button {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    height: 32px;
    padding: 0 12px 0 8px;
    border-radius: 8px;
    text-align: left;
    color: var(--text);
    transition: background-color 0.1s;
  }
  .shown button {
    animation: item 0.26s var(--ease-out) both;
    animation-delay: var(--d);
  }
  @keyframes item {
    from {
      opacity: 0;
      translate: 0 -3px;
    }
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: rgb(255 255 255 / 0.08);
    outline: none;
  }
  button:disabled {
    color: var(--text-3);
    cursor: default;
  }
  button.danger {
    color: #ff9196;
  }
  button.danger:hover:not(:disabled) {
    background: color-mix(in oklab, var(--danger) 18%, transparent);
  }
  .ic {
    width: 18px;
    display: grid;
    place-items: center;
    color: var(--text-2);
  }
  .check {
    color: var(--accent);
    display: grid;
  }
  .t {
    flex: 1;
    white-space: nowrap;
  }
  .hint {
    color: var(--text-3);
    font-size: 12px;
    padding-left: 16px;
  }
  .sep {
    height: 1px;
    margin: 5px 8px;
    background: rgb(255 255 255 / 0.08);
  }
  .lbl {
    padding: 8px 12px 4px 35px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.03em;
    color: var(--text-3);
  }
</style>
