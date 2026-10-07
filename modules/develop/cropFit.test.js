import { describe, it, expect } from "vitest";
import { fitInsideTurned, insideTurned } from "./cropFit.js";

const ASPECT = 1.5; // a 3:2 photo
const whole = { x: 0, y: 0, w: 1, h: 1 };

describe("insideTurned", () => {
  it("accepts any frame in an unturned picture, and rejects the whole frame in a turned one", () => {
    expect(insideTurned(whole, 0, ASPECT)).toBe(true);
    expect(insideTurned({ x: 0.2, y: 0.3, w: 0.5, h: 0.4 }, 0, ASPECT)).toBe(true);
    expect(insideTurned(whole, 5, ASPECT)).toBe(false);
  });

  it("accepts a small frame in the middle of a turned picture", () => {
    expect(insideTurned({ x: 0.4, y: 0.4, w: 0.2, h: 0.2 }, 10, ASPECT)).toBe(true);
  });
});

describe("fitInsideTurned", () => {
  it("leaves a frame that already fits exactly where it is", () => {
    const box = { x: 0.35, y: 0.35, w: 0.3, h: 0.3 };
    const fit = fitInsideTurned(box, 8, ASPECT);
    for (const k of ["x", "y", "w", "h"]) expect(fit[k]).toBeCloseTo(box[k], 9);
  });

  it("changes nothing when the picture is not turned", () => {
    const fit = fitInsideTurned(whole, 0, ASPECT);
    for (const k of ["x", "y", "w", "h"]) expect(fit[k]).toBeCloseTo(whole[k], 9);
  });

  it("shrinks the whole frame to the largest one that fits, in the same proportions", () => {
    for (const angle of [2, 5, 12, -7, -30, 45]) {
      const fit = fitInsideTurned(whole, angle, ASPECT);
      expect(insideTurned(fit, angle, ASPECT)).toBe(true);
      expect(fit.w).toBeLessThan(1);
      // Proportions held: pixel width / pixel height is still the photo's.
      expect((fit.w * ASPECT) / fit.h).toBeCloseTo(ASPECT, 9);
      // And it is the largest: a hair bigger would not fit.
      const k = 1.001;
      const bigger = { x: fit.x - (fit.w * (k - 1)) / 2, y: fit.y - (fit.h * (k - 1)) / 2, w: fit.w * k, h: fit.h * k };
      expect(insideTurned(bigger, angle, ASPECT)).toBe(false);
    }
  });

  it("slides a frame that fits in size but sticks out back inside, keeping its size", () => {
    const box = { x: 0.7, y: 0.6, w: 0.3, h: 0.3 }; // pushed into the bottom-right corner
    const fit = fitInsideTurned(box, 10, ASPECT);
    expect(fit.w).toBeCloseTo(box.w, 9);
    expect(fit.h).toBeCloseTo(box.h, 9);
    expect(insideTurned(fit, 10, ASPECT)).toBe(true);
    // It moved toward the middle, not away.
    expect(fit.x).toBeLessThan(box.x);
    expect(fit.y).toBeLessThan(box.y);
  });

  it("is symmetrical: the same turn the other way fits the same size", () => {
    const a = fitInsideTurned(whole, 9, ASPECT);
    const b = fitInsideTurned(whole, -9, ASPECT);
    expect(a.w).toBeCloseTo(b.w, 9);
    expect(a.h).toBeCloseTo(b.h, 9);
  });

  it("shrinks further as the turn grows", () => {
    const small = fitInsideTurned(whole, 3, ASPECT).w;
    const large = fitInsideTurned(whole, 20, ASPECT).w;
    expect(large).toBeLessThan(small);
  });
});
