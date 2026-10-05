import { describe, it, expect } from "vitest";
import { toPosition, fromPosition } from "./sliderScale.js";

describe("slider rail", () => {
  /** The bug as seen: saturation -1..0.5 put zero at two thirds of the rail. */
  it("puts a lopsided slider's default in the middle", () => {
    expect(toPosition(0, -1, 0.5, 0)).toBe(0.5);
    expect(toPosition(0, -40, 60, 0)).toBe(0.5);
  });

  it("keeps the ends at the ends", () => {
    expect(toPosition(-1, -1, 0.5, 0)).toBe(0);
    expect(toPosition(0.5, -1, 0.5, 0)).toBe(1);
  });

  it("scales each side on its own slope", () => {
    // Half of the way down to min is a quarter of the rail; half of the way
    // up to max is three quarters — whatever the two sides' lengths.
    expect(toPosition(-0.5, -1, 0.5, 0)).toBeCloseTo(0.25);
    expect(toPosition(0.25, -1, 0.5, 0)).toBeCloseTo(0.75);
  });

  it("stays linear when the default is an end of the range", () => {
    expect(toPosition(25, 0, 100, 0)).toBeCloseTo(0.25);
    expect(toPosition(25, 0, 100, undefined)).toBeCloseTo(0.25);
  });

  it("clamps values outside the range", () => {
    expect(toPosition(9, -1, 0.5, 0)).toBe(1);
    expect(toPosition(-9, -1, 0.5, 0)).toBe(0);
  });

  it("round-trips through a position", () => {
    for (const v of [-1, -0.37, 0, 0.12, 0.5]) {
      expect(fromPosition(toPosition(v, -1, 0.5, 0), -1, 0.5, 0, 0.01)).toBeCloseTo(v, 6);
    }
  });

  it("lands exactly on the default at the centre", () => {
    expect(fromPosition(0.5, -40, 60, 0, 0.5)).toBe(0);
    expect(fromPosition(0.5, 3000, 12000, 5500, 50)).toBe(5500);
  });

  it("snaps to the step without float drift", () => {
    expect(fromPosition(0.6, -0.5, 0.5, 0, 0.01)).toBe(0.1);
    expect(fromPosition(0.537, -40, 60, 0, 0.5)).toBe(4.5);
  });
});
