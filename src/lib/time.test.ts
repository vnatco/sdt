import { describe, expect, it } from "vitest";
import { ceilSec, clock, clockFromDigits, clockParts, dayWord, fromDigits, MAX_SEC, msUntil, nextTyped, short, spoken, split, wrapMin } from "./time";

describe("formatting", () => {
  it("splits seconds", () => {
    expect(split(3909)).toEqual({ h: 1, m: 5, s: 9 });
    expect(split(-5)).toEqual({ h: 0, m: 0, s: 0 });
  });
  it("formats a clock", () => {
    expect(clock(0)).toBe("0:00");
    expect(clock(9)).toBe("0:09");
    expect(clock(309)).toBe("5:09");
    expect(clock(3909)).toBe("1:05:09");
  });
  it("rounds remaining time up", () => {
    expect(ceilSec(0)).toBe(0);
    expect(ceilSec(1)).toBe(1);
    expect(ceilSec(1000)).toBe(1);
    expect(ceilSec(1001)).toBe(2);
  });
  it("speaks durations in Title Case", () => {
    expect(spoken(3600 + 60)).toBe("1 Hour 1 Minute");
    expect(spoken(45 * 60)).toBe("45 Minutes");
    expect(spoken(30)).toBe("30 Seconds");
    expect(spoken(0)).toBe("0 Seconds");
  });
  it("shortens durations", () => {
    expect(short(300)).toBe("5m");
    expect(short(3600)).toBe("1h");
    expect(short(5400)).toBe("1h 30m");
  });
});

describe("typed durations", () => {
  it("fills from the right with minutes last", () => {
    expect(fromDigits("5")).toBe(5 * 60);
    expect(fromDigits("45")).toBe(45 * 60);
    expect(fromDigits("130")).toBe(90 * 60);
    expect(fromDigits("90")).toBe(90 * 60);
    expect(fromDigits("1200")).toBe(12 * 3600);
  });
  it("keeps the last four digits and caps", () => {
    expect(fromDigits("123456")).toBe(34 * 3600 + 56 * 60);
    expect(fromDigits("9999")).toBe(Math.min(99 * 3600 + 99 * 60, MAX_SEC));
    expect(fromDigits("")).toBe(0);
  });
});

describe("typed clock times", () => {
  it("reads 24-hour digits", () => {
    expect(clockFromDigits("2330")).toEqual({ h: 23, m: 30 });
    expect(clockFromDigits("7")).toEqual({ h: 7, m: 0 });
    expect(clockFromDigits("730")).toEqual({ h: 7, m: 30 });
    expect(clockFromDigits("2460")).toBeNull();
    expect(clockFromDigits("2500")).toBeNull();
  });
  it("picks the next AM or PM on a 12-hour clock", () => {
    const evening = new Date(2026, 9, 5, 21, 0);
    expect(nextTyped(11, 30, evening, true)).toBe(23 * 60 + 30);
    const morning = new Date(2026, 9, 5, 8, 0);
    expect(nextTyped(11, 30, morning, true)).toBe(11 * 60 + 30);
    expect(nextTyped(12, 0, morning, true)).toBe(12 * 60);
    expect(nextTyped(11, 30, evening, false)).toBe(11 * 60 + 30);
    expect(nextTyped(15, 0, morning, true)).toBe(15 * 60);
  });
});

describe("clock targets", () => {
  it("counts to the next occurrence", () => {
    const now = new Date(2026, 9, 5, 22, 15, 30);
    expect(msUntil(23 * 60, now)).toBe((44 * 60 + 30) * 1000);
    expect(msUntil(22 * 60 + 15, now)).toBe(24 * 3600 * 1000 - 30 * 1000);
    expect(msUntil(1 * 60, now)).toBe((2 * 3600 + 44 * 60 + 30) * 1000);
  });
  it("wraps minutes around the day", () => {
    expect(wrapMin(-1)).toBe(1439);
    expect(wrapMin(1440)).toBe(0);
    expect(wrapMin(1441)).toBe(1);
  });
  it("shows 12 and 24 hour parts", () => {
    expect(clockParts(0, true)).toEqual({ hh: "12", mm: "00", ampm: "AM" });
    expect(clockParts(13 * 60 + 5, true)).toEqual({ hh: "1", mm: "05", ampm: "PM" });
    expect(clockParts(13 * 60 + 5, false)).toEqual({ hh: "13", mm: "05", ampm: null });
  });
  it("names the day", () => {
    const now = new Date(2026, 9, 5, 22, 0);
    expect(dayWord(new Date(2026, 9, 5, 23, 0), now)).toBe("Today");
    expect(dayWord(new Date(2026, 9, 6, 1, 0), now)).toBe("Tomorrow");
  });
});
