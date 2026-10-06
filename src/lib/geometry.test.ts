import { describe, expect, it } from "vitest";
import { anchorFor, CARD, compactWidth, CARD_WIN_H, COMPACT_RECT, dock, LINE_Y, MARGIN, MINI_WIN_H, PILL, pillRect, settleCard, settlePill, WIN_W, type Anchor } from "./geometry";

const WORK = { x: 0, y: 0, w: 1920, h: 1032 };
const ANCHORS: Anchor[] = (["left", "center", "right"] as const).flatMap((x) => (["top", "bottom"] as const).map((y) => ({ x, y })));

describe("layout", () => {
  it("puts the pill line at the dial centre inside the card", () => {
    expect(LINE_Y).toBeGreaterThan(CARD.y);
    expect(LINE_Y).toBeLessThan(CARD.y + CARD.h / 2);
    expect(CARD_WIN_H).toBe(CARD.h + 2 * MARGIN);
    expect(CARD.x + CARD.w / 2).toBe(WIN_W / 2);
  });
  it("fits every pill in the mini window in every direction", () => {
    for (const size of Object.keys(PILL) as (keyof typeof PILL)[]) {
      for (const anchor of ANCHORS) {
        const r = pillRect(size, anchor);
        expect(r.x).toBeGreaterThanOrEqual(MARGIN / 2);
        expect(r.x + r.w).toBeLessThanOrEqual(WIN_W - MARGIN / 2);
        expect(r.y).toBeGreaterThanOrEqual(MARGIN / 2);
        expect(r.y + r.h).toBeLessThanOrEqual(MINI_WIN_H - MARGIN / 2);
      }
    }
  });
  it("keeps the small pill in the same place for every anchor", () => {
    for (const anchor of ANCHORS) expect(pillRect("compact", anchor)).toEqual(COMPACT_RECT);
  });
  it("grows away from the anchored edge", () => {
    const right = pillRect("hover", { x: "right", y: "top" });
    expect(right.x + right.w).toBe(COMPACT_RECT.x + COMPACT_RECT.w);
    const bottom = pillRect("warning", { x: "left", y: "bottom" });
    expect(bottom.x).toBe(COMPACT_RECT.x);
    expect(bottom.y + bottom.h).toBe(COMPACT_RECT.y + COMPACT_RECT.h);
  });
});

describe("small pill width", () => {
  it("hugs the clock", () => {
    expect(compactWidth("5:09")).toBeLessThan(compactWidth("59:59"));
    expect(compactWidth("59:59")).toBeLessThan(compactWidth("1:00:00"));
    expect(compactWidth("99:59:59")).toBe(PILL.compact.w);
    // Ring, padding, gaps and icon (82) plus 0.6em digits and 0.3em colons at 19px.
    expect(compactWidth("5:09")).toBe(122);
    expect(compactWidth("1:00:00")).toBe(151);
  });
  it("keeps its anchored edge when it narrows", () => {
    const wide = pillRect("compact", { x: "right", y: "top" });
    const narrow = pillRect("compact", { x: "right", y: "top" }, compactWidth("5:09"));
    expect(narrow.x + narrow.w).toBe(wide.x + wide.w);
    const l = pillRect("compact", { x: "left", y: "top" }, compactWidth("5:09"));
    expect(l.x).toBe(COMPACT_RECT.x);
    const c = pillRect("compact", { x: "center", y: "top" }, compactWidth("5:09"));
    expect(c.x + c.w / 2).toBe(WIN_W / 2);
  });
});

describe("placement", () => {
  it("docks at the top centre", () => {
    const d = dock(WORK);
    expect(d.x + COMPACT_RECT.x + COMPACT_RECT.w / 2).toBe(960);
    expect(d.y + COMPACT_RECT.y).toBe(8);
  });
  it("snaps to the dock when dropped near it", () => {
    const d = dock(WORK);
    expect(settlePill(d.x + 30, d.y + 30, WORK)).toEqual(d);
  });
  it("keeps a free pill where it was dropped", () => {
    expect(settlePill(300, 400, WORK)).toEqual({ x: 300, y: 400 });
  });
  it("lets the small pill sit flush in a corner", () => {
    const s = settlePill(5000, 5000, WORK);
    expect(s.x + COMPACT_RECT.x + COMPACT_RECT.w).toBe(1920 - 8);
    expect(s.y + COMPACT_RECT.y + COMPACT_RECT.h).toBe(1032 - 8);
    expect(anchorFor(s.x, s.y, WORK)).toEqual({ x: "right", y: "bottom" });
    const t = settlePill(-5000, -5000, WORK);
    expect(t.x + COMPACT_RECT.x).toBe(8);
    expect(t.y + COMPACT_RECT.y).toBe(8);
    expect(anchorFor(t.x, t.y, WORK)).toEqual({ x: "left", y: "top" });
  });
  it("grows from the centre when there is room", () => {
    expect(anchorFor(dock(WORK).x, dock(WORK).y, WORK)).toEqual({ x: "center", y: "top" });
    expect(anchorFor(700, 900 - LINE_Y, WORK)).toEqual({ x: "center", y: "bottom" });
  });
  it("never grows off screen", () => {
    for (const [x, y] of [[-5000, -5000], [5000, 5000], [5000, -5000], [-5000, 5000], [700, 300]]) {
      const s = settlePill(x, y, WORK);
      const a = anchorFor(s.x, s.y, WORK);
      for (const size of Object.keys(PILL) as (keyof typeof PILL)[]) {
        const r = pillRect(size, a);
        expect(s.x + r.x).toBeGreaterThanOrEqual(0);
        expect(s.x + r.x + r.w).toBeLessThanOrEqual(1920);
        expect(s.y + r.y).toBeGreaterThanOrEqual(0);
        expect(s.y + r.y + r.h).toBeLessThanOrEqual(1032);
      }
    }
  });
  it("keeps the card on screen, also on a left monitor", () => {
    const s = settleCard(1800, 900, WORK);
    expect(s.x + CARD.x + CARD.w).toBeLessThanOrEqual(1920);
    expect(s.y + CARD.y + CARD.h).toBeLessThanOrEqual(1032);
    const left = { x: -1280, y: 0, w: 1280, h: 984 };
    const t = settleCard(-2000, -50, left);
    expect(t.x + CARD.x).toBeGreaterThanOrEqual(-1280);
    expect(t.y + CARD.y).toBeGreaterThanOrEqual(0);
  });
});
