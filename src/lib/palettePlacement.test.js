import { describe, it, expect } from "vitest";
import { placePalette, GAP, CASCADE } from "./palettePlacement.js";

const PANEL = 380;
/** A 2560-wide screen with a 1200-wide window sitting 200px in. */
const roomy = {
  main: { x: 200, y: 100, width: 1200 },
  monitor: { x: 0, width: 2560 },
};

describe("placing a palette", () => {
  it("puts it to the right of the main window when there is room", () => {
    const at = placePalette({ ...roomy, index: 0, panelWidth: PANEL });
    expect(at).toEqual({ x: 200 + 1200 + GAP, y: 100 });
  });

  it("falls back to the left when the right edge is too close", () => {
    // Window pushed right: 1600 + 1200 + 12 + 380 > 2560, but there are 1600
    // logical pixels free on the left.
    const at = placePalette({
      main: { x: 1600, y: 100, width: 900 },
      monitor: { x: 0, width: 2560 },
      index: 0,
      panelWidth: PANEL,
    });
    expect(at).toEqual({ x: 1600 - GAP - PANEL, y: 100 });
  });

  it("takes the second screen when neither side fits", () => {
    const at = placePalette({
      main: { x: 0, y: 0, width: 1440 },
      monitor: { x: 0, width: 1512 },
      otherMonitors: [{ x: 1512, y: 40 }],
      index: 0,
      panelWidth: PANEL,
    });
    expect(at).toEqual({ x: 1512 + GAP, y: 40 + GAP });
  });

  /**
   * One screen, and a window too wide for either side. Overlap cannot be
   * avoided, so it should at least be the same overlap every time rather
   * than a negative x that walks the panel off the screen.
   */
  it("pins to the right edge when there is nowhere else", () => {
    const at = placePalette({
      main: { x: 0, y: 0, width: 1440 },
      monitor: { x: 0, width: 1512 },
      index: 0,
      panelWidth: PANEL,
    });
    expect(at).toEqual({ x: 1512 - PANEL, y: 0 });
    expect(at.x).toBeGreaterThanOrEqual(0);
  });

  it("respects a monitor that does not start at zero", () => {
    const at = placePalette({
      main: { x: 2000, y: 50, width: 800 },
      monitor: { x: 1512, width: 2560 },
      index: 0,
      panelWidth: PANEL,
    });
    expect(at).toEqual({ x: 2000 + 800 + GAP, y: 50 });
  });

  it("cascades each further palette down and right", () => {
    const first = placePalette({ ...roomy, index: 0, panelWidth: PANEL });
    const third = placePalette({ ...roomy, index: 2, panelWidth: PANEL });
    expect(third.x - first.x).toBe(2 * CASCADE);
    expect(third.y - first.y).toBe(2 * CASCADE);
  });

  it("cascades on the second screen too", () => {
    const on = (/** @type {number} */ index) =>
      placePalette({
        main: { x: 0, y: 0, width: 1440 },
        monitor: { x: 0, width: 1512 },
        otherMonitors: [{ x: 1512, y: 0 }],
        index,
        panelWidth: PANEL,
      });
    expect(on(1).x - on(0).x).toBe(CASCADE);
    expect(on(1).y - on(0).y).toBe(CASCADE);
  });

  it("rounds to whole pixels, whatever the scale factor left behind", () => {
    const at = placePalette({
      main: { x: 100.4, y: 50.6, width: 800.3 },
      monitor: { x: 0, width: 2560 },
      index: 0,
      panelWidth: PANEL,
    });
    expect(Number.isInteger(at.x)).toBe(true);
    expect(Number.isInteger(at.y)).toBe(true);
  });

  /**
   * The boundary is inclusive: a palette that fits to the pixel goes right.
   *
   * The window has to be far enough in that the LEFT side is also available,
   * or this proves nothing — pinning to the right edge lands on exactly the
   * same x as an exact right-hand fit, by definition, so a version that got
   * the comparison wrong would still pass.
   */
  it("uses the right side when it fits exactly", () => {
    const at = placePalette({
      main: { x: 600, y: 0, width: 500 },
      monitor: { x: 0, width: 600 + 500 + GAP + PANEL },
      index: 0,
      panelWidth: PANEL,
    });
    expect(at.x).toBe(600 + 500 + GAP);
  });
});
