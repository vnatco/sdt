<script lang="ts">
  // The full view: set a time on the dial, choose what happens, start.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { Action } from "./api";
  import Dial from "./Dial.svelte";
  import Digits from "./ui/Digits.svelte";
  import Icon from "./ui/Icon.svelte";
  import { tip } from "./ui/tip";
  import { ACTIONS, actionInfo, app } from "./state.svelte";
  import { clock, clockParts, dayWord, short, timeOfDay, twelveHour } from "./time";

  let {
    confirmQuit = $bindable(false),
    about = $bindable(false),
    onstart,
    onpause,
    onresume,
    oncancel,
    onextend,
    onaction,
    onshrink,
    onhide,
    onclose,
    onquit,
  }: {
    confirmQuit?: boolean;
    about?: boolean;
    onstart: () => void;
    onpause: () => void;
    onresume: () => void;
    oncancel: () => void;
    onextend: (sec: number) => void;
    onaction: (a: Action) => void;
    onshrink: () => void;
    onhide: () => void;
    onclose: () => void;
    onquit: () => void;
  } = $props();

  const twelve = twelveHour();
  const info = $derived(actionInfo(app.action));
  const actionIndex = $derived(ACTIONS.findIndex((a) => a.id === app.action));
  const setting = $derived(app.idle);
  const atMode = $derived(setting && app.settings.mode === "at");
  const parts = $derived(clockParts(app.settings.at, twelve));
  const digits = $derived(atMode ? `${parts.hh}:${parts.mm}` : clock(app.seconds));
  const CHIPS = [5 * 60, 15 * 60, 30 * 60, 60 * 60];
  // Each second of the warning beats the ring.
  const beat = $derived(app.warning ? app.seconds : 0);

  const caption = $derived.by(() => {
    switch (app.phase) {
      case "running":
        return `${info.doing} In`;
      case "paused":
        return "Paused";
      case "firing":
        return `${info.doing}…`;
      default:
        return "";
    }
  });

  const sub = $derived.by(() => {
    const end = app.endsAt;
    if (app.phase === "paused") return "Resume When You're Ready";
    if (app.phase === "firing") return app.dryRun ? "Dry Run" : "Closing Apps";
    if (!end) return "Set a Time";
    if (atMode) {
      const d = dayWord(end, new Date(app.now));
      return `In ${short(Math.ceil(app.ms / 60000) * 60)}${d === "Today" ? "" : ` · ${d}`}`;
    }
    const d = dayWord(end, new Date(app.now));
    return `${setting ? `${info.label} at` : "At"} ${timeOfDay(end)}${d === "Today" ? "" : ` ${d}`}`;
  });

  // Dial input snaps to whole minutes; fractional turns accumulate.
  let carry = 0;
  function turn(min: number, jumpTo: number | null) {
    if (jumpTo !== null) {
      carry = 0;
      if (app.settings.mode === "in") {
        const hours = Math.floor(app.settings.duration / 3600);
        const m = Math.round(jumpTo) % 60;
        app.setDuration(hours * 3600 + (m === 0 && hours === 0 ? 60 : m * 60));
      } else {
        const base = Math.floor(app.settings.at / 60) * 60;
        app.setAt(base + (Math.round(jumpTo) % 60));
      }
      return;
    }
    carry += min;
    const whole = Math.trunc(carry);
    if (whole === 0) return;
    carry -= whole;
    if (app.settings.mode === "in") {
      // Stop at zero rather than winding into negative time.
      const next = Math.max(0, Math.round(app.settings.duration / 60) + whole);
      app.setDuration(next * 60);
    } else {
      app.setAt(app.settings.at + whole);
    }
  }

  function wheelGroup(unit: "h" | "m" | "s", delta: number) {
    if (!setting) return;
    if (atMode) app.setAt(app.settings.at + delta * (unit === "h" ? 60 : 1));
    else app.setDuration(app.settings.duration + delta * (unit === "h" ? 3600 : unit === "m" ? 60 : 5));
  }

  function setMode(mode: "in" | "at") {
    if (mode === app.settings.mode) return;
    if (mode === "at") {
      // Start from the moment the current duration would end, rounded to 5 min.
      const end = new Date(Date.now() + app.settings.duration * 1000);
      const m = end.getHours() * 60 + end.getMinutes();
      app.set({ mode, at: Math.round(m / 5) * 5 % 1440 });
    } else {
      app.set({ mode, duration: Math.max(60, Math.round(app.ms / 60000) * 60) });
    }
  }

  function chip(sec: number) {
    if (setting) app.nudge(sec);
    else onextend(sec);
  }

</script>

<div class="card" class:flipped={about}>
  <div class="face front" inert={about || confirmQuit}>
    <header>
      <div class="brand">
        <span class="glyph"><Icon name="shutdown" size={13} stroke={2.4} /></span>
        <span class="title">Shut Down Timer</span>
        {#if app.dryRun}<span class="badge" use:tip={"Power actions are only logged"}>Dry Run</span>{/if}
      </div>
      <div class="tools">
        <button class="ib" class:on={app.settings.onTop} onclick={() => app.set({ onTop: !app.settings.onTop })} use:tip={app.settings.onTop ? "Stop Keeping on Top" : "Keep on Top"}>
          <Icon name="pin" size={16} />
        </button>
        <button class="ib" disabled={!app.active} onclick={onshrink} use:tip={"Shrink to Pill"}>
          <Icon name="pill" size={16} />
        </button>
        <button class="ib" onclick={onhide} use:tip={"Hide to Tray"}>
          <Icon name="tray" size={16} />
        </button>
        <button class="ib close" onclick={onclose} use:tip={app.active ? "Close" : "Quit"}>
          <Icon name="close" size={16} />
        </button>
      </div>
    </header>

    <div class="dial-wrap">
      <Dial
        value={setting ? app.seconds / 3600 : app.fraction}
        wrap={setting}
        interactive={setting}
        paused={app.phase === "paused"}
        warning={app.warning}
        pulse={beat}
        onturn={turn}
        onstep={(m) => app.nudge(m * 60)}
      >
        <div class="readout" class:blink={app.phase === "paused"}>
          <div class="top">
            {#if setting}
              <div class="mode" role="tablist" aria-label="Timer Mode" in:fade={{ duration: 200 }}>
                <span class="mode-ind" class:at={app.settings.mode === "at"}></span>
                <button role="tab" aria-selected={app.settings.mode === "in"} class:sel={app.settings.mode === "in"} onclick={() => setMode("in")} use:tip={{ text: "Count Down a Duration", side: "top" }}>In</button>
                <button role="tab" aria-selected={app.settings.mode === "at"} class:sel={app.settings.mode === "at"} onclick={() => setMode("at")} use:tip={{ text: "Pick a Time of Day", side: "top" }}>At</button>
              </div>
            {:else}
              <span class="caption" in:fade={{ duration: 200 }}>{caption}</span>
            {/if}
          </div>
          <div class="big" class:firing={app.phase === "firing"}>
            {#if app.phase === "firing"}
              <span class="spinner"></span>
            {:else}
              <Digits text={digits} dir={setting ? "up" : "down"} groups={setting} onwheelgroup={wheelGroup} />
              {#if atMode && parts.ampm}<span class="ampm">{parts.ampm}</span>{/if}
            {/if}
          </div>
          <div class="sub">{sub}</div>
        </div>
      </Dial>
    </div>

    <div class="chips">
      {#each CHIPS as c (c)}
        <button class="chip" disabled={app.phase === "firing"} onclick={() => chip(c)} use:tip={setting ? `Add ${short(c)}` : `Add ${short(c)} to the Timer`}>
          <Icon name="plus" size={12} stroke={2.4} />{short(c)}
        </button>
      {/each}
      {#if setting}
        <button class="chip icon" onclick={() => (app.settings.mode === "in" ? app.setDuration(0) : setMode("at"))} use:tip={"Reset"} transition:fade={{ duration: 150 }}>
          <Icon name="reset" size={14} stroke={2} />
        </button>
      {/if}
    </div>

    <div class="actions" role="radiogroup" aria-label="When Time is Up" style:--i={actionIndex}>
      <span class="ind"></span>
      {#each ACTIONS as a (a.id)}
        <button role="radio" aria-checked={app.action === a.id} class:sel={app.action === a.id} disabled={app.phase === "firing"} onclick={() => onaction(a.id)}>
          <Icon name={a.icon} size={16} stroke={2} />
          <span>{a.label}</span>
        </button>
      {/each}
    </div>

    <div class="primary">
      {#if setting}
        <button class="go" disabled={app.seconds <= 0} onclick={onstart} in:fly={{ y: 8, duration: 260, easing: cubicOut }}>
          <span class="shine"></span>
          <Icon name="play" size={16} fill />
          <span>Start</span>
        </button>
      {:else if app.phase === "firing"}
        <button class="go" disabled>
          <span>{info.doing}…</span>
        </button>
      {:else}
        <div class="pair" in:fly={{ y: 8, duration: 260, easing: cubicOut }}>
          {#if app.phase === "paused"}
            <button class="go" onclick={onresume}>
              <span class="shine"></span>
              <Icon name="play" size={16} fill />
              <span>Resume</span>
            </button>
          {:else}
            <button class="soft" onclick={onpause}>
              <Icon name="pause" size={16} stroke={2.4} />
              <span>Pause</span>
            </button>
          {/if}
          <button class="soft danger" onclick={oncancel}>
            <Icon name="x" size={16} stroke={2.2} />
            <span>Cancel</span>
          </button>
        </div>
      {/if}
    </div>
  </div>

  <div class="face back" inert={!about}>
    <header>
      <div class="brand"><span class="title">About</span></div>
      <div class="tools">
        <button class="ib" onclick={() => (about = false)} use:tip={"Back"}>
          <Icon name="close" size={16} />
        </button>
      </div>
    </header>
    <div class="about">
      <div class="logo">
        <svg viewBox="0 0 1024 1024" width="96" height="96" aria-hidden="true">
          <defs>
            <linearGradient id="about-arc" gradientUnits="userSpaceOnUse" x1="720" y1="260" x2="260" y2="700">
              <stop offset="0" stop-color="#FFC75F" />
              <stop offset="0.5" stop-color="#FF7A4F" />
              <stop offset="1" stop-color="#F2416B" />
            </linearGradient>
          </defs>
          <path d="M 655.1 311 A 270 270 0 1 1 368.9 311" fill="none" stroke="#fff" stroke-opacity="0.1" stroke-width="84" stroke-linecap="round" />
          <path class="logo-arc" d="M 655.1 311 A 270 270 0 1 1 258.3 632.3" fill="none" stroke="url(#about-arc)" stroke-width="84" stroke-linecap="round" />
          <rect x="470" y="172" width="84" height="340" rx="42" fill="#fff" />
        </svg>
      </div>
      <h1>Shut Down Timer</h1>
      <p class="ver">Version {app.version}</p>
      <p class="blurb">Shuts down, restarts or puts your computer to sleep when time is up. Shut down and restart are forced: open apps are closed without asking to save.</p>
      <div class="links">
        <button class="soft" onclick={() => app.guard(openUrl("https://github.com/vnatco/sdt"))}>
          <Icon name="github" size={16} />
          <span>Source Code</span>
        </button>
      </div>
      <p class="fine">© 2026 Vladimer Natchkepia · Apache License 2.0</p>
    </div>
  </div>

  {#if confirmQuit}
    <div class="sheet-bg" transition:fade={{ duration: 180 }}></div>
    <div class="sheet" role="alertdialog" aria-labelledby="quit-title" transition:fly={{ y: 24, duration: 320, easing: cubicOut }}>
      <h2 id="quit-title">Quit and Cancel the Timer?</h2>
      <p>The computer won't {info.label.toLowerCase()} if Shut Down Timer quits now. Hide it in the tray to keep the countdown going.</p>
      <div class="pair">
        <button class="go" onclick={() => { confirmQuit = false; onhide(); }}>
          <Icon name="tray" size={16} />
          <span>Keep in Tray</span>
        </button>
        <button class="soft danger" onclick={onquit}>
          <span>Quit</span>
        </button>
      </div>
      <button class="link" onclick={() => (confirmQuit = false)}>Never Mind</button>
    </div>
  {/if}

  {#if app.toast}
    {#key app.toast.id}
      <div class="toast" class:err={app.toast.error} role="status" in:fly={{ y: 16, duration: 320, easing: cubicOut }} out:fade={{ duration: 160 }}>
        <span class="dot"></span>
        <span class="msg">{app.toast.text}</span>
        <button class="ib tiny" onclick={() => app.dismissToast()} aria-label="Dismiss"><Icon name="x" size={13} stroke={2.2} /></button>
      </div>
    {/key}
  {/if}
</div>

<style>
  .card {
    position: absolute;
    inset: 0;
    perspective: 1400px;
  }
  .face {
    position: absolute;
    inset: 0;
    padding: 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    backface-visibility: hidden;
    transition: transform 0.7s var(--spring-soft);
  }
  .back {
    transform: rotateY(-180deg);
  }
  .flipped .front {
    transform: rotateY(180deg);
  }
  .flipped .back {
    transform: rotateY(0deg);
  }

  header {
    width: 100%;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 2px;
  }
  .glyph {
    width: 20px;
    height: 20px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    color: var(--on-accent);
    background: linear-gradient(135deg, var(--accent-2), var(--accent));
    box-shadow: 0 0 12px color-mix(in oklab, var(--accent) 50%, transparent);
  }
  .title {
    font-size: 12.5px;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--text-2);
  }
  .badge {
    font-size: 10.5px;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 99px;
    color: var(--accent);
    background: color-mix(in oklab, var(--accent) 16%, transparent);
  }
  .tools {
    display: flex;
    gap: 2px;
  }
  .ib {
    width: 30px;
    height: 30px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    color: var(--text-2);
    transition:
      background-color 0.15s,
      color 0.15s,
      scale 0.15s;
  }
  .ib:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .ib:active:not(:disabled) {
    scale: 0.92;
  }
  .ib:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .ib.on {
    color: var(--accent);
    background: color-mix(in oklab, var(--accent) 14%, transparent);
  }
  .ib.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  .ib.tiny {
    width: 22px;
    height: 22px;
    border-radius: 6px;
  }

  .dial-wrap {
    flex: none;
  }
  .readout {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    pointer-events: auto;
  }
  .top {
    height: 26px;
    display: grid;
    place-items: center;
  }
  .caption {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--text-2);
  }
  .mode {
    position: relative;
    display: flex;
    padding: 2px;
    border-radius: 99px;
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .mode button {
    position: relative;
    z-index: 1;
    width: 40px;
    height: 22px;
    border-radius: 99px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-3);
    transition: color 0.25s;
  }
  .mode button.sel {
    color: var(--text);
  }
  .mode-ind {
    position: absolute;
    left: 2px;
    top: 2px;
    width: 40px;
    height: 22px;
    border-radius: 99px;
    background: rgb(255 255 255 / 0.12);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.3);
    transition: translate 0.4s var(--spring);
  }
  .mode-ind.at {
    translate: 40px 0;
  }
  .big {
    height: 56px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 50px;
    font-weight: 300;
    letter-spacing: -0.01em;
    color: var(--text);
  }
  .ampm {
    font-family: var(--font-num);
    font-size: 15px;
    font-weight: 500;
    color: var(--text-2);
    margin-left: 4px;
    align-self: flex-start;
    margin-top: 10px;
  }
  .blink .big {
    animation: blink 1.6s ease-in-out infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .sub {
    height: 16px;
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .spinner {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    border: 3px solid var(--line-2);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }

  .chips {
    margin-top: 16px;
    height: 34px;
    display: flex;
    gap: 6px;
  }
  .chip {
    height: 32px;
    padding: 0 11px 0 9px;
    border-radius: 99px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-2);
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line);
    transition:
      background-color 0.15s,
      color 0.15s,
      box-shadow 0.15s,
      scale 0.2s var(--spring);
  }
  .chip.icon {
    width: 32px;
    padding: 0;
    justify-content: center;
  }
  .chip:hover:not(:disabled) {
    color: var(--text);
    background: color-mix(in oklab, var(--accent) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent) 45%, transparent);
  }
  .chip:active:not(:disabled) {
    scale: 0.93;
  }
  .chip:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .actions {
    position: relative;
    margin-top: 16px;
    width: 100%;
    height: 46px;
    padding: 4px;
    border-radius: 15px;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .actions .ind {
    position: absolute;
    left: 4px;
    top: 4px;
    bottom: 4px;
    width: calc((100% - 8px) / 3);
    border-radius: 11px;
    translate: calc(var(--i) * 100%) 0;
    background: color-mix(in oklab, var(--accent) 20%, rgb(255 255 255 / 0.04));
    box-shadow:
      inset 0 0 0 1px color-mix(in oklab, var(--accent) 50%, transparent),
      0 4px 18px color-mix(in oklab, var(--accent) 22%, transparent);
    transition: translate 0.45s var(--spring);
  }
  .actions button {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border-radius: 11px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-3);
    transition: color 0.3s;
  }
  .actions button:hover:not(.sel):not(:disabled) {
    color: var(--text-2);
  }
  .actions button.sel {
    color: var(--text);
  }
  .actions button.sel :global(svg) {
    color: var(--accent);
    filter: drop-shadow(0 0 6px color-mix(in oklab, var(--accent) 70%, transparent));
  }

  .primary {
    margin-top: auto;
    width: 100%;
    height: 50px;
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    width: 100%;
  }
  .go,
  .soft {
    position: relative;
    width: 100%;
    height: 50px;
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    font-size: 14.5px;
    font-weight: 700;
    letter-spacing: 0.01em;
    overflow: hidden;
    transition:
      scale 0.25s var(--spring),
      filter 0.2s,
      box-shadow 0.3s,
      background-color 0.2s;
  }
  .go {
    color: var(--on-accent);
    background: linear-gradient(120deg, var(--accent-2), var(--accent));
    box-shadow:
      0 8px 26px -6px color-mix(in oklab, var(--accent) 75%, transparent),
      inset 0 1px 0 rgb(255 255 255 / 0.35);
  }
  .go:hover:not(:disabled) {
    filter: brightness(1.07);
    box-shadow:
      0 12px 34px -6px color-mix(in oklab, var(--accent) 90%, transparent),
      inset 0 1px 0 rgb(255 255 255 / 0.35);
  }
  .go:active:not(:disabled),
  .soft:active {
    scale: 0.97;
  }
  .go:disabled {
    background: var(--surface-2);
    color: var(--text-3);
    box-shadow: none;
    cursor: default;
  }
  .shine {
    position: absolute;
    inset: 0;
    background: linear-gradient(105deg, transparent 35%, rgb(255 255 255 / 0.45) 50%, transparent 65%);
    translate: -110% 0;
  }
  .go:hover:not(:disabled) .shine {
    translate: 110% 0;
    transition: translate 0.8s var(--ease-out);
  }
  .soft {
    color: var(--text);
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--line);
    font-weight: 600;
  }
  .soft:hover {
    background: var(--hover);
  }
  .soft.danger {
    color: #ff8a8f;
  }
  .soft.danger:hover {
    background: color-mix(in oklab, var(--danger) 18%, transparent);
    color: #ffb3b6;
  }

  /* About */
  .about {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 18px 10px 0;
  }
  .logo {
    width: 112px;
    height: 112px;
    border-radius: 30px;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, #2b2735, #0d0c12);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 0.12),
      0 18px 40px rgb(0 0 0 / 0.5),
      0 0 50px -10px rgb(255 106 77 / 0.5);
  }
  .flipped .logo-arc {
    stroke-dasharray: 1200;
    animation: draw 1.4s 0.3s var(--ease-out) both;
  }
  @keyframes draw {
    from {
      stroke-dashoffset: 1200;
    }
  }
  h1 {
    margin: 20px 0 0;
    font-size: 21px;
    font-weight: 700;
  }
  .ver {
    margin: 4px 0 0;
    color: var(--text-3);
    font-size: 12px;
  }
  .blurb {
    margin: 18px 0 0;
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.55;
  }
  .links {
    margin-top: 22px;
    width: 180px;
  }
  .links .soft {
    height: 40px;
    border-radius: 12px;
    font-size: 13px;
  }
  .fine {
    margin-top: auto;
    margin-bottom: 4px;
    color: var(--text-3);
    font-size: 11.5px;
  }

  /* Quit sheet */
  .sheet-bg {
    position: absolute;
    inset: 0;
    z-index: 20;
    border-radius: 28px;
    background: rgb(10 9 14 / 0.6);
    backdrop-filter: blur(6px);
  }
  .sheet {
    position: absolute;
    left: 14px;
    z-index: 21;
    right: 14px;
    bottom: 14px;
    padding: 22px 20px 12px;
    border-radius: 22px;
    background: #211f29;
    box-shadow:
      0 0 0 1px var(--line-2),
      0 24px 60px rgb(0 0 0 / 0.6);
    text-align: center;
  }
  .sheet h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
  }
  .sheet p {
    margin: 10px 0 18px;
    color: var(--text-2);
    line-height: 1.5;
  }
  .sheet .go,
  .sheet .soft {
    height: 44px;
    border-radius: 13px;
    font-size: 13.5px;
  }
  .link {
    margin-top: 8px;
    height: 32px;
    padding: 0 12px;
    color: var(--text-2);
    font-weight: 600;
    border-radius: 8px;
  }
  .link:hover {
    color: var(--text);
    background: var(--hover);
  }

  /* Toast */
  .toast {
    position: absolute;
    left: 16px;
    right: 16px;
    bottom: 84px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 8px 9px 14px;
    border-radius: 14px;
    background: #26242e;
    box-shadow:
      0 0 0 1px var(--line-2),
      0 14px 34px rgb(0 0 0 / 0.5);
    font-size: 12.5px;
    line-height: 1.4;
  }
  .toast .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 10px var(--accent);
  }
  .toast.err .dot {
    background: var(--danger);
    box-shadow: 0 0 10px var(--danger);
  }
  .toast .msg {
    flex: 1;
  }
</style>
