<script lang="ts">
  // The timer window: boot, the morph between the card and the pill, window
  // motion, the menu, keyboard and sounds.
  //
  // Card -> pill: the card's shape shrinks into the pill around the dial's
  // centre while the window glides to where the pill lives; then the window
  // drops its transparent bottom (top-left fixed, so nothing moves).
  // Pill -> card: the window gets the transparent space back first, then the
  // shape blooms into the card while the window glides to where it fits.
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { api, events, type Action, type Frame, type MenuItem, type Rect } from "./api";
  import { ACTIONS, app } from "./state.svelte";
  import {
    anchorFor,
    CARD,
    CARD_CENTER_DY,
    CARD_WIN_H,
    centerCard,
    compactWidth,
    LINE_Y,
    MINI_WIN_H,
    NO_ANCHOR,
    PILL,
    pillRect,
    settleCard,
    settlePill,
    WIN_W,
    type Anchor,
    type PillSize,
  } from "./geometry";
  import Card from "./Card.svelte";
  import Pill from "./Pill.svelte";
  import { chime, tick as tickSound, unlock } from "./sound";
  import { clock, clockFromDigits, fromDigits, msUntil, nextTyped, short, twelveHour } from "./time";
  import { hideTip } from "./ui/tip";

  const MORPH_MS = 560;
  /** --spring-soft in app.css; the drag's slide follows it. */
  const SPRING_SOFT: [number, number, number, number] = [0.3, 1.18, 0.5, 1];
  const GLIDE_MS = 640;
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

  let view = $state<"card" | "pill">("card");
  let morph = $state<null | "shrink" | "glide" | "toCard">(null);
  let anchor = $state<Anchor>(NO_ANCHOR);
  let hover = $state(false);
  /** The pill is being dragged: it stays small, so it lands exactly where it's dropped. */
  let carrying = $state(false);
  let hidden = $state(true);
  let entering = $state(false);
  let leaving = $state(false);
  let confirmQuit = $state(false);
  let about = $state(false);
  let leaveTimer: ReturnType<typeof setTimeout> | undefined;

  const pillSize = $derived<PillSize>(
    app.phase === "firing"
      ? "firing"
      : app.warning
        ? "warning"
        : carrying
          ? "compact"
          : app.toast
            ? "notice"
            : hover
              ? "hover"
              : "compact",
  );

  // The small pill hugs its clock.
  const compactW = $derived(compactWidth(clock(app.seconds)));

  // The shape: the card, or the pill in its current size.
  const shape = $derived.by((): { rect: Rect; r: number } => {
    if (view === "card") return { rect: CARD, r: 28 };
    // Shrinking into the pill lands on the small shape; blooming starts from
    // whatever the pill is showing.
    const small = morph === "shrink" || morph === "glide";
    const size = small ? "compact" : pillSize;
    return { rect: pillRect(size, small ? NO_ANCHOR : anchor, compactW), r: PILL[size].r };
  });

  const geo = $derived(`left:${shape.rect.x}px;top:${shape.rect.y}px;width:${shape.rect.w}px;height:${shape.rect.h}px;border-radius:${shape.r}px`);
  const clip = $derived(
    `clip-path: inset(${shape.rect.y}px ${WIN_W - shape.rect.x - shape.rect.w}px ${CARD_WIN_H - shape.rect.y - shape.rect.h}px ${shape.rect.x}px round ${shape.r}px)`,
  );
  const geoMs = $derived(morph ? MORPH_MS : 440);

  // ---- Click-through -------------------------------------------------------
  $effect(() => {
    const r = shape.rect;
    const rects = morph || view === "card" ? [CARD] : [{ x: r.x - 2, y: r.y - 2, w: r.w + 4, h: r.h + 4 }];
    api.hit(rects).catch((e) => console.error("hit rects", e));
  });

  // ---- Theme -----------------------------------------------------------------
  $effect(() => {
    document.documentElement.dataset.action = app.action;
    if (app.warning) document.documentElement.dataset.warning = "";
    else delete document.documentElement.dataset.warning;
  });

  // ---- Window role (taskbar, on top) -----------------------------------------
  $effect(() => {
    const pill = view === "pill";
    const onTop = app.settings.onTop || app.warning;
    if (!app.ready || morph) return;
    api.mode(pill, onTop).catch((e) => app.notify(`Can't Change the Window: ${e}`, true));
  });

  // ---- Clock -----------------------------------------------------------------
  // Every frame while the ring is moving visibly, otherwise once a second.
  $effect(() => {
    if (app.phase !== "running") {
      const id = setInterval(() => (app.now = Date.now()), 1000);
      return () => clearInterval(id);
    }
    let raf = 0;
    let last = 0;
    const step = Math.max(16, Math.min(1000, app.timer.totalMs / 1440));
    const loop = () => {
      const t = Date.now();
      if (t - last >= step || Math.ceil((app.timer.deadline! - t) / 1000) !== Math.ceil((app.timer.deadline! - last) / 1000)) {
        app.now = t;
        last = t;
      }
      raf = requestAnimationFrame(loop);
    };
    loop();
    // A hidden window gets no frames; keep the clock right for when it returns.
    const id = setInterval(() => (app.now = Date.now()), 1000);
    return () => {
      cancelAnimationFrame(raf);
      clearInterval(id);
    };
  });

  // ---- Sounds ------------------------------------------------------------------
  let wasWarning = false;
  let lastTick = -1;
  $effect(() => {
    const warn = app.warning;
    const s = app.seconds;
    if (app.settings.sound && app.phase === "running") {
      if (warn && !wasWarning && s > 10) chime();
      if (warn && s >= 1 && s <= 10 && s !== lastTick) tickSound(s === 1);
    }
    wasWarning = warn;
    lastTick = s;
  });

  // ---- Morph -------------------------------------------------------------------
  /** Where the pill goes: where the user last put it, or the card's centre. */
  async function pillHome(f: Frame): Promise<{ x: number; y: number }> {
    const p = app.settings.pillPos;
    if (p && (await api.onScreen(p.x + WIN_W / 2, p.y + LINE_Y).catch(() => false))) return p;
    return settlePill(f.x, f.y + CARD_CENTER_DY, f.work, false);
  }

  async function toPill() {
    if (view === "pill" || morph || !app.active) return;
    hideTip();
    about = false;
    confirmQuit = false;
    hover = false;
    const f = await app.guard(api.frame(WIN_W / 2, LINE_Y));
    if (!f) return;
    app.set({ cardPos: { x: f.x, y: f.y } });
    const home = await pillHome(f);
    morph = "shrink";
    view = "pill";
    // The pill's face fades in once the shape is nearly small.
    const faceIn = setTimeout(() => morph === "shrink" && (morph = "glide"), MORPH_MS * 0.28);
    try {
      await Promise.all([api.glide(home.x, home.y, GLIDE_MS), sleep(MORPH_MS)]);
      const g = await api.frame(WIN_W / 2, LINE_Y);
      await api.setFrame({ x: g.x, y: g.y, w: WIN_W, h: MINI_WIN_H });
      await api.mode(true, true);
      anchor = anchorFor(g.x, g.y, g.work);
    } catch (e) {
      app.notify(`Can't Move the Window: ${e}`, true);
    } finally {
      clearTimeout(faceIn);
      morph = null;
    }
  }

  async function toCard() {
    if (view === "card" || morph) return;
    hideTip();
    hover = false;
    carrying = false;
    morph = "toCard";
    try {
      const f = await api.frame(WIN_W / 2, LINE_Y);
      // Bloom around the pill, then settle where the whole card fits.
      const t = settleCard(f.x, f.y - CARD_CENTER_DY, f.work);
      await api.setFrame({ x: f.x, y: f.y, w: WIN_W, h: CARD_WIN_H });
      await api.mode(false, app.settings.onTop || app.warning);
      view = "card";
      await Promise.all([api.glide(t.x, t.y, GLIDE_MS), sleep(MORPH_MS)]);
    } catch (e) {
      view = "card";
      app.notify(`Can't Move the Window: ${e}`, true);
    } finally {
      morph = null;
    }
  }

  // ---- Dragging ----------------------------------------------------------------
  async function drag(e: PointerEvent) {
    if (e.button !== 0 || morph) return;
    if ((e.target as HTMLElement).closest("button, [role=slider], a")) return;
    hideTip();
    // A grabbed pill shrinks toward the cursor, keeping the spot under it.
    const grab = view === "pill" ? { from: shape.rect, to: pillRect("compact", anchor, compactW), ms: geoMs, ease: SPRING_SOFT } : null;
    const moved = await app.guard(api.drag(grab));
    if (!moved) {
      carrying = false;
      if (moved === false && view === "pill" && !app.warning) toCard();
      return;
    }
    const f = await app.guard(api.frame(WIN_W / 2, LINE_Y));
    if (!f) {
      carrying = false;
      return;
    }
    if (view === "pill") {
      const s = settlePill(f.x, f.y, f.work);
      if (s.x !== f.x || s.y !== f.y) await app.guard(api.glide(s.x, s.y, 420));
      anchor = anchorFor(s.x, s.y, f.work);
      app.set({ pillPos: s });
      carrying = false;
    } else {
      const s = settleCard(f.x, f.y, f.work);
      if (s.x !== f.x || s.y !== f.y) await app.guard(api.glide(s.x, s.y, 420));
      app.set({ cardPos: s });
    }
  }

  // Hover makes the pill grow; leaving shrinks it after a forgiving pause.
  function pointer(inside: boolean) {
    clearTimeout(leaveTimer);
    if (view !== "pill" || morph) {
      hover = false;
      return;
    }
    if (inside) hover = true;
    else leaveTimer = setTimeout(() => (hover = false), 420);
  }

  // ---- Show / hide ---------------------------------------------------------------
  // Every hide fades out first and stays faded while hidden: a window shows
  // its last painted frame when it comes back, and that must be empty, or
  // the card flashes up before its entrance plays.
  async function hideToTray() {
    if (hidden || leaving) return;
    hideTip();
    confirmQuit = false;
    hover = false;
    leaving = true;
    await sleep(220);
    try {
      await api.hide();
      hidden = true;
    } catch (e) {
      leaving = false;
      app.notify(`Can't Hide the Window: ${e}`, true);
    }
  }

  function onShown() {
    hidden = false;
    hover = false;
    leaving = false;
    entering = true;
    setTimeout(() => (entering = false), 650);
  }

  async function showWindow() {
    if (!hidden) return;
    await app.guard(api.show());
    onShown();
  }

  // ---- Timer actions -------------------------------------------------------------
  async function start() {
    unlock();
    const ms = app.settings.mode === "in" ? app.settings.duration * 1000 : msUntil(app.settings.at, new Date());
    const s = await app.guard(api.start(ms, app.settings.action));
    if (!s) return;
    app.timer = s;
    entry = "";
    if (app.settings.autoMini) {
      setTimeout(() => {
        if (app.active && view === "card" && !hidden) toPill();
      }, 900);
    }
  }

  async function update(p: Promise<typeof app.timer>) {
    const s = await app.guard(p);
    if (s) app.timer = s;
  }

  const pause = () => update(api.pause());
  const resume = () => {
    unlock();
    update(api.resume());
  };
  const cancel = () => update(api.cancel());
  const extend = (sec: number) => update(api.extend(sec * 1000));

  function setAction(a: Action) {
    app.set({ action: a });
    update(api.setAction(a));
  }

  function close() {
    if (app.active) confirmQuit = true;
    else api.quit();
  }

  // ---- Menu ------------------------------------------------------------------------
  function menuItems(): MenuItem[] {
    const items: MenuItem[] = [];
    const firing = app.phase === "firing";
    if (app.active) {
      if (app.phase === "paused") items.push({ kind: "item", id: "resume", label: "Resume", icon: "play" });
      else items.push({ kind: "item", id: "pause", label: "Pause", icon: "pause" });
      items.push(
        { kind: "item", id: "add:300", label: "Add 5 Minutes", icon: "plus" },
        { kind: "item", id: "add:900", label: "Add 15 Minutes", icon: "plus" },
        { kind: "item", id: "add:3600", label: "Add 1 Hour", icon: "plus" },
        { kind: "item", id: "cancel", label: "Cancel Timer", icon: "x", danger: true },
      );
    } else if (app.idle) {
      items.push({ kind: "item", id: "start", label: "Start Timer", icon: "play", hint: app.seconds > 0 ? short(Math.ceil(app.seconds / 60) * 60) : undefined, disabled: app.seconds <= 0 });
    }
    items.push({ kind: "sep" }, { kind: "label", label: "When Time is Up" });
    for (const a of ACTIONS) items.push({ kind: "item", id: `action:${a.id}`, label: a.label, checked: app.action === a.id, disabled: firing });
    items.push({ kind: "sep" });
    if (hidden) {
      items.push({ kind: "item", id: "show", label: "Show Timer", icon: "eye" });
    } else {
      if (view === "pill") items.push({ kind: "item", id: "card", label: "Open Full View", icon: "expand" });
      else items.push({ kind: "item", id: "pill", label: "Shrink to Pill", icon: "pill", disabled: !app.active });
      items.push({ kind: "item", id: "hide", label: "Hide to Tray", icon: "tray" });
    }
    items.push(
      { kind: "sep" },
      { kind: "label", label: "Options" },
      { kind: "item", id: "opt:onTop", label: "Keep on Top", checked: app.settings.onTop },
      { kind: "item", id: "opt:autoMini", label: "Shrink to Pill on Start", checked: app.settings.autoMini },
      { kind: "item", id: "opt:sound", label: "Warning Sound", checked: app.settings.sound },
      { kind: "item", id: "opt:keepAwake", label: "Keep the Computer Awake", checked: app.settings.keepAwake },
      { kind: "sep" },
      { kind: "item", id: "about", label: "About Shut Down Timer", icon: "info" },
      { kind: "item", id: "quit", label: app.active ? "Quit and Cancel Timer" : "Quit", icon: "quit", danger: app.active },
    );
    return items;
  }

  function openMenu() {
    hideTip();
    api.menuOpen(menuItems(), app.action).catch((e) => app.notify(`Can't Open the Menu: ${e}`, true));
  }

  async function pick(id: string) {
    if (id.startsWith("add:")) return extend(Number(id.slice(4)));
    if (id.startsWith("action:")) return setAction(id.slice(7) as Action);
    if (id.startsWith("opt:")) {
      const k = id.slice(4) as "onTop" | "autoMini" | "sound" | "keepAwake";
      app.set({ [k]: !app.settings[k] });
      if (k === "sound" && app.settings.sound) unlock();
      return;
    }
    switch (id) {
      case "start":
        await showWindow();
        return start();
      case "pause":
        return pause();
      case "resume":
        return resume();
      case "cancel":
        return cancel();
      case "show":
        return showWindow();
      case "card":
        return toCard();
      case "pill":
        return toPill();
      case "hide":
        return hideToTray();
      case "about":
        await showWindow();
        if (view === "pill") await toCard();
        confirmQuit = false;
        about = true;
        return;
      case "quit":
        return api.quit();
    }
  }

  // ---- Keyboard ------------------------------------------------------------------
  let entry = "";
  let entryTimer: ReturnType<typeof setTimeout> | undefined;

  function typeDigits(next: string) {
    entry = next.slice(-4);
    clearTimeout(entryTimer);
    entryTimer = setTimeout(() => (entry = ""), 2500);
    if (app.settings.mode === "in") {
      app.setDuration(fromDigits(entry));
    } else {
      const t = clockFromDigits(entry);
      if (t) app.setAt(nextTyped(t.h, t.m, new Date(), twelveHour()));
    }
  }

  function key(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === "Escape") {
      if (confirmQuit) confirmQuit = false;
      else if (about) about = false;
      else if (view === "card" && app.active) toPill();
      return;
    }
    if (confirmQuit || about || morph) return;
    const onButton = (e.target as HTMLElement).tagName === "BUTTON";
    if (view === "pill") {
      if (e.key === " " && !onButton) {
        e.preventDefault();
        if (app.phase === "running") pause();
        else if (app.phase === "paused") resume();
      } else if (e.key === "Enter" && !onButton) toCard();
      return;
    }
    if (/^[0-9]$/.test(e.key) && app.idle) {
      e.preventDefault();
      typeDigits(entry + e.key);
    } else if (e.key === "Backspace" && app.idle) {
      e.preventDefault();
      if (entry) typeDigits(entry.slice(0, -1));
      else if (app.settings.mode === "in") app.setDuration(Math.floor(app.settings.duration / 600) * 60);
    } else if ((e.key === "Enter" || e.key === " ") && !onButton) {
      e.preventDefault();
      if (app.idle) start();
      else if (app.phase === "running") pause();
      else if (app.phase === "paused") resume();
    } else if ((e.key === "ArrowUp" || e.key === "ArrowDown") && app.idle) {
      e.preventDefault();
      app.nudge((e.key === "ArrowUp" ? 1 : -1) * (e.shiftKey ? 300 : 60));
    } else if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && app.phase !== "firing" && !onButton) {
      e.preventDefault();
      const i = ACTIONS.findIndex((a) => a.id === app.action);
      setAction(ACTIONS[(i + (e.key === "ArrowRight" ? 1 : ACTIONS.length - 1)) % ACTIONS.length].id);
    }
  }

  // ---- Boot ------------------------------------------------------------------------
  onMount(() => {
    const unsubs = [
      events.timer((s) => {
        const was = app.timer.phase;
        app.timer = s;
        // Done, cancelled or failed while small: come back to the card.
        if (was !== "idle" && s.phase === "idle" && view === "pill") toCard();
      }),
      events.notice((n) => {
        app.notify(n.text, n.error);
        if (n.error) showWindow();
      }),
      events.pointerInside(pointer),
      events.trayMenu(openMenu),
      events.menuPick((id) => pick(id)),
      events.shown(onShown),
      events.trayHide(hideToTray),
      events.dragMoved(() => {
        if (view === "pill") carrying = true;
      }),
    ];

    (async () => {
      const b = await app.guard(api.boot());
      if (b) {
        // Always open on In; the last clock time is still remembered for At.
        app.settings = { ...b.settings, mode: "in" };
        app.timer = b.timer;
        app.version = b.version;
        app.dryRun = b.dryRun;
      }
      try {
        const f = await api.frame(WIN_W / 2, LINE_Y);
        let pos = centerCard(f.work);
        const saved = app.settings.cardPos;
        if (saved && (await api.onScreen(saved.x + WIN_W / 2, saved.y + LINE_Y))) {
          const g = await api.frame(WIN_W / 2, LINE_Y);
          pos = settleCard(saved.x, saved.y, g.work);
        }
        await api.setFrame({ x: pos.x, y: pos.y, w: WIN_W, h: CARD_WIN_H });
        await api.mode(false, app.settings.onTop);
      } catch (e) {
        app.notify(`Can't Place the Window: ${e}`, true);
      }
      app.ready = true;
      // Show once the first frame is painted (no flash of an empty window).
      requestAnimationFrame(() =>
        requestAnimationFrame(async () => {
          await app.guard(api.show());
          onShown();
        }),
      );
    })();

    return () => unsubs.forEach((u) => u.then((f) => f()));
  });
</script>

<svelte:window
  onkeydown={key}
  oncontextmenu={(e) => {
    e.preventDefault();
    openMenu();
  }}
/>

<div class="stage" class:ready={app.ready} class:entering class:leaving style:--geo="{geoMs}ms" style:width="{WIN_W}px" style:height="{CARD_WIN_H}px">
  <div class="body" class:pill={view === "pill"} class:warn={app.warning && view === "pill"} style={geo}>
    <div class="aurora" class:on={view === "card"}>
      <span class="blob a"></span>
      <span class="blob b"></span>
    </div>
  </div>

  <div class="content" style="{clip};width:{WIN_W}px;height:{CARD_WIN_H}px" onpointerdown={drag} role="presentation">
    <div class="card-box" class:off={view === "pill"} inert={view === "pill"} style:left="{CARD.x}px" style:top="{CARD.y}px" style:width="{CARD.w}px" style:height="{CARD.h}px">
      <Card
        bind:confirmQuit
        bind:about
        onstart={start}
        onpause={pause}
        onresume={resume}
        oncancel={cancel}
        onextend={extend}
        onaction={setAction}
        onshrink={toPill}
        onhide={hideToTray}
        onclose={close}
        onquit={() => api.quit()}
      />
    </div>

    {#if view === "pill"}
      <div class="pill-box" class:settled={morph !== "shrink"} style={geo} out:fade={{ duration: 140 }}>
        <Pill size={morph === "shrink" || morph === "glide" ? "compact" : pillSize} onpause={pause} onresume={resume} oncancel={cancel} onextend={extend} onexpand={toCard} onhide={hideToTray} />
      </div>
    {/if}
  </div>
</div>

<style>
  .stage {
    position: fixed;
    left: 0;
    top: 0;
    opacity: 0;
  }
  .stage.ready {
    opacity: 1;
  }
  .stage.entering .body,
  .stage.entering .content {
    animation: enter 0.55s var(--spring) both;
    transform-origin: 50% 222px;
  }
  @keyframes enter {
    from {
      opacity: 0;
      scale: 0.9;
    }
  }
  .stage.leaving .body,
  .stage.leaving .content {
    transition:
      opacity 0.18s ease-in,
      scale 0.2s ease-in;
    opacity: 0;
    scale: 0.94;
    transform-origin: 50% 222px;
  }

  .body {
    position: absolute;
    overflow: hidden;
    background: linear-gradient(180deg, var(--bg-1), var(--bg-2));
    box-shadow:
      inset 0 0 0 1px rgb(255 255 255 / 0.07),
      inset 0 1px 0 rgb(255 255 255 / 0.06),
      0 26px 60px -14px rgb(0 0 0 / 0.75),
      0 0 0 1px rgb(0 0 0 / 0.45);
    transition:
      left var(--geo) var(--spring-soft),
      top var(--geo) var(--spring-soft),
      width var(--geo) var(--spring-soft),
      height var(--geo) var(--spring-soft),
      border-radius var(--geo) var(--spring-soft),
      box-shadow 0.5s;
  }
  .body.pill {
    background: linear-gradient(180deg, #121118, #0a090d);
    box-shadow:
      inset 0 0 0 1px rgb(255 255 255 / 0.09),
      inset 0 1px 0 rgb(255 255 255 / 0.07),
      0 10px 28px -6px rgb(0 0 0 / 0.7),
      0 0 22px -8px color-mix(in oklab, var(--accent) 70%, transparent);
  }
  .body.warn {
    animation: alarm 1s ease-in-out infinite;
  }
  @keyframes alarm {
    50% {
      box-shadow:
        inset 0 0 0 1px color-mix(in oklab, var(--accent) 60%, transparent),
        0 10px 28px -6px rgb(0 0 0 / 0.7),
        0 0 30px -2px color-mix(in oklab, var(--accent) 80%, transparent);
    }
  }

  /* Slow coloured light drifting behind the card. */
  .aurora {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity 0.6s;
  }
  .aurora.on {
    opacity: 1;
  }
  .blob {
    position: absolute;
    width: 340px;
    height: 340px;
    border-radius: 50%;
  }
  .blob.a {
    left: -110px;
    top: -150px;
    background: radial-gradient(closest-side, color-mix(in oklab, var(--accent-2) 26%, transparent), transparent);
    animation: drift-a 16s ease-in-out infinite alternate;
  }
  .blob.b {
    right: -140px;
    bottom: -130px;
    background: radial-gradient(closest-side, color-mix(in oklab, var(--accent) 24%, transparent), transparent);
    animation: drift-b 19s ease-in-out infinite alternate;
  }
  @keyframes drift-a {
    to {
      translate: 120px 90px;
      scale: 1.2;
    }
  }
  @keyframes drift-b {
    to {
      translate: -110px -70px;
      scale: 0.9;
    }
  }

  .content {
    position: absolute;
    left: 0;
    top: 0;
    transition: clip-path var(--geo) var(--spring-soft);
  }
  .card-box {
    position: absolute;
    transition:
      opacity 0.26s ease-out 0.04s,
      scale 0.5s var(--spring-soft) 0.06s;
    transform-origin: 50% 194px;
  }
  .card-box.off {
    opacity: 0;
    scale: 0.88;
    transition:
      opacity 0.16s ease-in,
      scale 0.45s ease-in;
  }
  .pill-box {
    position: absolute;
    overflow: hidden;
    opacity: 0;
    transition:
      left var(--geo) var(--spring-soft),
      top var(--geo) var(--spring-soft),
      width var(--geo) var(--spring-soft),
      height var(--geo) var(--spring-soft),
      border-radius var(--geo) var(--spring-soft),
      opacity 0.24s ease-out 0.04s;
  }
  .pill-box.settled {
    opacity: 1;
  }
</style>
