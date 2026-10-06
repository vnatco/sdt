// Time maths and formatting. Pure, so it is tested on its own.

export const MAX_SEC = 99 * 3600 + 59 * 60 + 59;
export const DAY_MIN = 24 * 60;

export interface HMS {
  h: number;
  m: number;
  s: number;
}

export function split(sec: number): HMS {
  const t = Math.max(0, Math.floor(sec));
  return { h: Math.floor(t / 3600), m: Math.floor(t / 60) % 60, s: t % 60 };
}

/** Whole seconds left, rounded up, so the display never shows 0:00 early. */
export function ceilSec(ms: number): number {
  return Math.max(0, Math.ceil(ms / 1000 - 1e-6));
}

/** 1:05:09, 5:09 or 0:09. */
export function clock(sec: number): string {
  const { h, m, s } = split(sec);
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** "1 Hour 5 Minutes", "45 Minutes", "30 Seconds": for labels and the screen reader. */
export function spoken(sec: number): string {
  const { h, m, s } = split(sec);
  const parts: string[] = [];
  if (h) parts.push(`${h} ${h === 1 ? "Hour" : "Hours"}`);
  if (m) parts.push(`${m} ${m === 1 ? "Minute" : "Minutes"}`);
  if (s && !h) parts.push(`${s} ${s === 1 ? "Second" : "Seconds"}`);
  return parts.join(" ") || "0 Seconds";
}

/** Short form for chips and buttons: 5m, 1h, 1h 30m. */
export function short(sec: number): string {
  const { h, m } = split(sec);
  if (h && m) return `${h}h ${m}m`;
  if (h) return `${h}h`;
  return `${m}m`;
}

/**
 * Typed digits fill the time from the right, like a microwave, with minutes
 * as the last two digits: "5" is 5 minutes, "130" is 1:30:00, "90" is 1:30:00.
 */
export function fromDigits(digits: string): number {
  const d = digits.replace(/\D/g, "").slice(-4);
  if (!d) return 0;
  const n = Number(d);
  const sec = Math.floor(n / 100) * 3600 + (n % 100) * 60;
  return Math.min(sec, MAX_SEC);
}

/** Typed digits as a clock time, 24-hour: "2330" is 23:30, "7" is 07:00. Null if impossible. */
export function clockFromDigits(digits: string): { h: number; m: number } | null {
  const d = digits.replace(/\D/g, "").slice(-4);
  if (!d) return null;
  let h: number;
  let m: number;
  if (d.length <= 2) {
    h = Number(d);
    m = 0;
  } else {
    h = Number(d.slice(0, -2));
    m = Number(d.slice(-2));
  }
  if (h > 23 || m > 59) return null;
  return { h, m };
}

/**
 * Minutes after midnight for a typed time. On a 12-hour clock "11:30" could
 * be morning or night; pick whichever comes next.
 */
export function nextTyped(h: number, m: number, now: Date, twelveHour: boolean): number {
  const target = h * 60 + m;
  if (!twelveHour || h === 0 || h > 12) return target;
  const am = (h % 12) * 60 + m;
  const pm = am + 12 * 60;
  return msUntil(am, now) <= msUntil(pm, now) ? am : pm;
}

/** Milliseconds from `now` until the next time the clock shows `atMin` (always > 0). */
export function msUntil(atMin: number, now: Date): number {
  const t = new Date(now);
  t.setHours(Math.floor(atMin / 60), atMin % 60, 0, 0);
  let ms = t.getTime() - now.getTime();
  if (ms <= 0) {
    t.setDate(t.getDate() + 1);
    t.setHours(Math.floor(atMin / 60), atMin % 60, 0, 0);
    ms = t.getTime() - now.getTime();
  }
  return ms;
}

export function wrapMin(min: number): number {
  return ((Math.round(min) % DAY_MIN) + DAY_MIN) % DAY_MIN;
}

export function clampSec(sec: number): number {
  return Math.max(0, Math.min(MAX_SEC, Math.round(sec)));
}

let twelve: boolean | undefined;
export function twelveHour(): boolean {
  if (twelve === undefined) {
    const hc = new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions().hourCycle;
    twelve = hc === "h11" || hc === "h12";
  }
  return twelve;
}

/** A clock time in the user's format: "11:42 PM" or "23:42". */
export function timeOfDay(date: Date): string {
  return new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" }).format(date);
}

/** Parts for the big clock-time display in At mode. */
export function clockParts(atMin: number, twelveH: boolean): { hh: string; mm: string; ampm: string | null } {
  const h = Math.floor(atMin / 60);
  const m = atMin % 60;
  if (!twelveH) return { hh: String(h).padStart(2, "0"), mm: String(m).padStart(2, "0"), ampm: null };
  const h12 = h % 12 === 0 ? 12 : h % 12;
  return { hh: String(h12), mm: String(m).padStart(2, "0"), ampm: h < 12 ? "AM" : "PM" };
}

/** "Today", "Tomorrow" or the weekday of a deadline. */
export function dayWord(target: Date, now: Date): string {
  const a = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const b = new Date(target.getFullYear(), target.getMonth(), target.getDate()).getTime();
  const days = Math.round((b - a) / 86_400_000);
  if (days <= 0) return "Today";
  if (days === 1) return "Tomorrow";
  return new Intl.DateTimeFormat(undefined, { weekday: "long" }).format(target);
}
