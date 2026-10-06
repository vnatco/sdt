// App state shared by the card and the pill.

import { api, errorText, type Action, type Settings, type Snapshot } from "./api";
import { ceilSec, clampSec, msUntil, wrapMin } from "./time";

export const DEFAULT_SETTINGS: Settings = {
  action: "shutdown",
  mode: "in",
  duration: 30 * 60,
  at: 23 * 60,
  cardPos: null,
  pillPos: null,
  onTop: false,
  autoMini: false,
  sound: true,
  keepAwake: true,
};

const IDLE: Snapshot = {
  phase: "idle",
  remainingMs: 0,
  totalMs: 0,
  deadline: null,
  now: 0,
  action: "shutdown",
  warning: false,
  late: false,
  dryRun: false,
};

export const ACTIONS: { id: Action; label: string; doing: string; icon: string }[] = [
  { id: "shutdown", label: "Shut Down", doing: "Shutting Down", icon: "shutdown" },
  { id: "restart", label: "Restart", doing: "Restarting", icon: "restart" },
  { id: "sleep", label: "Sleep", doing: "Going to Sleep", icon: "sleep" },
];

export function actionInfo(a: Action) {
  return ACTIONS.find((x) => x.id === a) ?? ACTIONS[0];
}

class Store {
  settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  timer = $state<Snapshot>({ ...IDLE });
  version = $state("");
  dryRun = $state(false);
  ready = $state(false);
  /** Ticks every frame while a timer runs, every second otherwise. */
  now = $state(Date.now());
  toast = $state<{ id: number; text: string; error: boolean } | null>(null);
  #toastTimer: ReturnType<typeof setTimeout> | undefined;
  #saveTimer: ReturnType<typeof setTimeout> | undefined;

  get phase() {
    return this.timer.phase;
  }
  get active() {
    return this.timer.phase === "running" || this.timer.phase === "paused";
  }
  get idle() {
    return this.timer.phase === "idle";
  }

  /** Exact milliseconds left (or set, when idle). */
  get ms(): number {
    const t = this.timer;
    if (t.phase === "running" && t.deadline) return Math.max(0, t.deadline - this.now);
    if (t.phase === "paused") return t.remainingMs;
    if (t.phase === "firing") return 0;
    if (this.settings.mode === "in") return this.settings.duration * 1000;
    return msUntil(this.settings.at, new Date(this.now));
  }

  /** Whole seconds shown on the clock. */
  get seconds(): number {
    return ceilSec(this.ms);
  }

  /** Ring fill: time left of the whole run, or (when setting) the minutes past the hour. */
  get fraction(): number {
    const t = this.timer;
    if (this.active || t.phase === "firing") return t.totalMs > 0 ? Math.min(1, this.ms / t.totalMs) : 0;
    const sec = this.seconds;
    if (sec <= 0) return 0;
    const inHour = sec % 3600;
    return inHour === 0 ? 1 : inHour / 3600;
  }


  get warning(): boolean {
    return this.timer.phase === "running" && (this.timer.warning || this.ms <= 60_000);
  }

  get action(): Action {
    return this.active || this.timer.phase === "firing" ? this.timer.action : this.settings.action;
  }

  /** When the timer will act, as a date. */
  get endsAt(): Date | null {
    if (this.timer.phase === "running" && this.timer.deadline) return new Date(this.timer.deadline);
    if (this.timer.phase === "paused") return null;
    if (this.idle && this.seconds > 0) return new Date(this.now + this.ms);
    return null;
  }

  set(patch: Partial<Settings>) {
    Object.assign(this.settings, patch);
    clearTimeout(this.#saveTimer);
    this.#saveTimer = setTimeout(() => {
      api.saveSettings($state.snapshot(this.settings)).catch((e) => this.notify(`Settings Not Saved: ${errorText(e)}`, true));
    }, 350);
  }

  setDuration(sec: number) {
    this.set({ duration: clampSec(sec) });
  }

  setAt(min: number) {
    this.set({ at: wrapMin(min) });
  }

  /** Add (or remove) time to what is being set. */
  nudge(sec: number) {
    if (this.settings.mode === "in") this.setDuration(this.settings.duration + sec);
    else this.setAt(this.settings.at + sec / 60);
  }

  notify(text: string, error = false) {
    clearTimeout(this.#toastTimer);
    this.toast = { id: Date.now(), text, error };
    this.#toastTimer = setTimeout(() => (this.toast = null), error ? 7000 : 4500);
  }

  dismissToast() {
    clearTimeout(this.#toastTimer);
    this.toast = null;
  }

  /** Runs a backend call, reporting failure to the user. */
  async guard<T>(p: Promise<T>): Promise<T | undefined> {
    try {
      return await p;
    } catch (e) {
      console.error(e);
      this.notify(errorText(e), true);
      return undefined;
    }
  }
}

export const app = new Store();
