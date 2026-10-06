// Window and shape geometry, in logical px.
//
// One window serves both views. It is always WIN_W wide; the card view is
// CARD_WIN_H tall, the pill view MINI_WIN_H. Both share the top-left corner
// and the small pill's centre (WIN_W / 2, LINE_Y), so switching the window
// between the two heights never moves anything on screen: only transparent
// space is added or removed at the bottom.
//
// The small pill sits in the middle of the window, with room on every side
// to grow. Near a screen edge it grows away from that edge (the anchors), so
// the small pill itself can sit right against the edge or in a corner.

import type { Rect } from "./api";

export const MARGIN = 28;

export type PillSize = "compact" | "hover" | "notice" | "firing" | "warning";
export const PILL: Record<PillSize, { w: number; h: number; r: number }> = {
  compact: { w: 212, h: 44, r: 22 },
  hover: { w: 340, h: 58, r: 29 },
  firing: { w: 264, h: 52, r: 26 },
  notice: { w: 360, h: 66, r: 26 },
  warning: { w: 372, h: 108, r: 34 },
};
const COMPACT = PILL.compact;
const TALLEST = Math.max(...Object.values(PILL).map((p) => p.h));
const WIDEST = Math.max(...Object.values(PILL).map((p) => p.w));
/** How far a pill can reach past the small one on one side. */
const REACH_X = WIDEST - COMPACT.w;
const REACH_Y = TALLEST - COMPACT.h;

export const WIN_W = COMPACT.w + 2 * REACH_X + 2 * MARGIN;
export const CARD: Rect = { x: (WIN_W - 360) / 2, y: MARGIN, w: 360, h: 540 };
export const CARD_WIN_H = CARD.h + 2 * MARGIN;
export const DIAL = 272;
/** The small pill's centre line: the dial's centre (header 32 + gap 6 below the 20 padding). */
export const LINE_Y = CARD.y + 20 + 32 + 6 + DIAL / 2;
/** From the dial's centre to the card's centre. */
export const CARD_CENTER_DY = CARD.y + CARD.h / 2 - LINE_Y;

export const MINI_WIN_H = LINE_Y + COMPACT.h / 2 + REACH_Y + MARGIN;

export type AnchorY = "top" | "bottom";
export type AnchorX = "left" | "center" | "right";
export interface Anchor {
  x: AnchorX;
  y: AnchorY;
}
export const NO_ANCHOR: Anchor = { x: "center", y: "top" };

/** The small pill, in window coordinates. */
export const COMPACT_RECT: Rect = { x: (WIN_W - COMPACT.w) / 2, y: LINE_Y - COMPACT.h / 2, w: COMPACT.w, h: COMPACT.h };

/** Where a pill of the given size sits in the window. */
export function pillRect(size: PillSize, anchor: Anchor): Rect {
  const p = PILL[size];
  const c = COMPACT_RECT;
  const x = anchor.x === "left" ? c.x : anchor.x === "right" ? c.x + c.w - p.w : (WIN_W - p.w) / 2;
  const y = anchor.y === "top" ? c.y : c.y + c.h - p.h;
  return { x, y, w: p.w, h: p.h };
}

const EDGE = 8;
const SNAP = 48;

/** Grow away from the nearer edges. */
export function anchorFor(winX: number, winY: number, work: Rect): Anchor {
  const left = winX + COMPACT_RECT.x - work.x;
  const right = work.x + work.w - (winX + COMPACT_RECT.x + COMPACT.w);
  const room = REACH_X / 2 + EDGE;
  const x: AnchorX = left < room && left <= right ? "left" : right < room ? "right" : "center";
  const y: AnchorY = winY + LINE_Y < work.y + work.h / 2 ? "top" : "bottom";
  return { x, y };
}

/** Centred at the top of the screen, like an island. */
export function dock(work: Rect): { x: number; y: number } {
  return { x: Math.round(work.x + (work.w - WIN_W) / 2), y: Math.round(work.y + EDGE - COMPACT_RECT.y) };
}

/**
 * Where a dropped pill should settle: snapped to the top centre when dropped
 * close to it, otherwise where it is, pulled fully onto the screen.
 */
export function settlePill(x: number, y: number, work: Rect, snap = true): { x: number; y: number } {
  const home = dock(work);
  const left = x + COMPACT_RECT.x;
  const top = y + COMPACT_RECT.y;
  if (snap && Math.abs(x - home.x) < SNAP && top - work.y < SNAP) return home;
  const l = Math.min(Math.max(left, work.x + EDGE), work.x + work.w - EDGE - COMPACT.w);
  const t = Math.min(Math.max(top, work.y + EDGE), work.y + work.h - EDGE - COMPACT.h);
  return { x: Math.round(l - COMPACT_RECT.x), y: Math.round(t - COMPACT_RECT.y) };
}

/** Keep the card fully on screen. */
export function settleCard(x: number, y: number, work: Rect): { x: number; y: number } {
  const cx = Math.min(Math.max(x + CARD.x, work.x + EDGE), work.x + work.w - EDGE - CARD.w);
  const cy = Math.min(Math.max(y + CARD.y, work.y + EDGE), work.y + work.h - EDGE - CARD.h);
  return { x: Math.round(cx - CARD.x), y: Math.round(cy - CARD.y) };
}

/** The card centred on the screen. */
export function centerCard(work: Rect): { x: number; y: number } {
  return { x: Math.round(work.x + (work.w - WIN_W) / 2), y: Math.round(work.y + (work.h - CARD_WIN_H) / 2) };
}
