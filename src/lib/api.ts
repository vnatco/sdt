// Typed access to the backend: commands and events.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Action = "shutdown" | "restart" | "sleep";
export type Phase = "idle" | "running" | "paused" | "firing";

export interface Snapshot {
  phase: Phase;
  remainingMs: number;
  totalMs: number;
  deadline: number | null;
  now: number;
  action: Action;
  warning: boolean;
  late: boolean;
  dryRun: boolean;
}

export interface Point {
  x: number;
  y: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Settings {
  action: Action;
  mode: "in" | "at";
  duration: number;
  at: number;
  cardPos: Point | null;
  pillPos: Point | null;
  onTop: boolean;
  autoMini: boolean;
  sound: boolean;
  keepAwake: boolean;
}

export interface Boot {
  settings: Settings;
  timer: Snapshot;
  version: string;
  dryRun: boolean;
}

export interface Frame extends Rect {
  scale: number;
  work: Rect;
}

export interface Notice {
  error: boolean;
  text: string;
}

export type MenuItem =
  | { kind: "item"; id: string; label: string; hint?: string; checked?: boolean; disabled?: boolean; danger?: boolean; icon?: string }
  | { kind: "sep" }
  | { kind: "label"; label: string };

export const api = {
  boot: () => invoke<Boot>("boot"),
  saveSettings: (settings: Settings) => invoke<void>("settings_set", { settings }),
  start: (ms: number, action: Action) => invoke<Snapshot>("timer_start", { ms, action }),
  pause: () => invoke<Snapshot>("timer_pause"),
  resume: () => invoke<Snapshot>("timer_resume"),
  extend: (ms: number) => invoke<Snapshot>("timer_extend", { ms }),
  cancel: () => invoke<Snapshot>("timer_cancel"),
  setAction: (action: Action) => invoke<Snapshot>("timer_set_action", { action }),
  quit: () => invoke<void>("app_quit"),

  hit: (rects: Rect[]) => invoke<void>("window_hit", { rects }),
  frame: (ax: number, ay: number) => invoke<Frame>("window_frame", { ax, ay }),
  setFrame: (r: Rect) => invoke<void>("window_set_frame", { ...r }),
  glide: (x: number, y: number, ms: number) => invoke<void>("window_glide", { x, y, ms }),
  drag: (grab: { from: Rect; to: Rect; ms: number; ease: [number, number, number, number] } | null) => invoke<boolean>("window_drag", { grab }),
  mode: (pill: boolean, onTop: boolean) => invoke<void>("window_mode", { pill, onTop }),
  show: () => invoke<void>("window_show"),
  hide: () => invoke<void>("window_hide"),
  onScreen: (x: number, y: number) => invoke<boolean>("point_on_screen", { x, y }),

  menuOpen: (items: MenuItem[], action: Action) => invoke<void>("menu_open", { items, action }),
  menuPlace: (w: number, h: number) => invoke<{ flipX: boolean; flipY: boolean }>("menu_place", { w, h }),
  menuPick: (id: string) => invoke<void>("menu_pick", { id }),
  menuClose: () => invoke<void>("menu_close"),
};

export const events = {
  timer: (f: (s: Snapshot) => void): Promise<UnlistenFn> => listen<Snapshot>("timer", (e) => f(e.payload)),
  notice: (f: (n: Notice) => void): Promise<UnlistenFn> => listen<Notice>("notice", (e) => f(e.payload)),
  pointerInside: (f: (inside: boolean) => void): Promise<UnlistenFn> => listen<{ inside: boolean }>("pointer-inside", (e) => f(e.payload.inside)),
  trayMenu: (f: () => void): Promise<UnlistenFn> => listen("tray-menu", () => f()),
  menuPick: (f: (id: string) => void): Promise<UnlistenFn> => listen<string>("menu-pick", (e) => f(e.payload)),
  menuShow: (f: (m: { items: MenuItem[]; action: Action }) => void): Promise<UnlistenFn> => listen<{ items: MenuItem[]; action: Action }>("menu-show", (e) => f(e.payload)),
  shown: (f: () => void): Promise<UnlistenFn> => listen("window-shown", () => f()),
  trayHide: (f: () => void): Promise<UnlistenFn> => listen("tray-hide", () => f()),
  dragMoved: (f: () => void): Promise<UnlistenFn> => listen("drag-moved", () => f()),
};

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
