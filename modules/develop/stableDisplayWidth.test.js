import { describe, it, expect } from "vitest";
import { nextStableWidth, displayBox, NO_STABLE_WIDTH } from "./stableDisplayWidth.js";

describe("nextStableWidth", () => {
  it("keeps the widest render of the same shape, so a small live render is not drawn smaller", () => {
    let s = nextStableWidth(NO_STABLE_WIDTH, 2048, 1365);
    expect(s.w).toBe(2048);
    s = nextStableWidth(s, 768, 512); // a drag render, same shape
    expect(s.w).toBe(2048);
    s = nextStableWidth(s, 2048, 1365);
    expect(s.w).toBe(2048);
  });

  it("follows the first render of a photo, then grows to the settled one", () => {
    let s = nextStableWidth(NO_STABLE_WIDTH, 640, 427); // the thumbnail proxy
    expect(s.w).toBe(640);
    s = nextStableWidth(s, 2048, 1365);
    expect(s.w).toBe(2048);
  });

  it("draws a photo at its settled size from the first live render, before the settled one lands", () => {
    // A slider moved the moment the photo opened: the thumbnail, then drag renders at 768.
    let s = nextStableWidth(NO_STABLE_WIDTH, 768, 512); // the grid's thumbnail
    expect(s.w).toBe(768);
    s = nextStableWidth(s, 768, 512, true); // a live render
    expect(s.w).toBe(2048);
    // And a portrait stands for its settled width, not for 2048.
    const p = nextStableWidth(NO_STABLE_WIDTH, 512, 768, true);
    expect(p.w).toBeCloseTo(1365, 0);
  });

  it("starts over when the shape changes", () => {
    let s = nextStableWidth(NO_STABLE_WIDTH, 2048, 1365);
    s = nextStableWidth(s, 1000, 1365); // a crop
    expect(s.w).toBe(1000);
  });

  it("ignores an image with no size", () => {
    const s = nextStableWidth({ key: "1.500", w: 2048 }, 0, 0);
    expect(s).toEqual({ key: "1.500", w: 2048 });
  });
});

describe("displayBox", () => {
  const stable = (w, h) => nextStableWidth(NO_STABLE_WIDTH, w, h);

  it("fits a landscape photo by the frame's width", () => {
    const b = displayBox({ frameW: 1300, frameH: 1000, percent: 85, stable: stable(2048, 1365) });
    expect(b.w).toBeCloseTo(1105, 0);
    expect(b.h).toBeCloseTo(1105 / 1.5, 0);
  });

  it("fits a portrait photo by the frame's HEIGHT and keeps its shape", () => {
    const b = displayBox({ frameW: 1300, frameH: 1000, percent: 85, stable: stable(1365, 2048) });
    expect(b.h).toBeCloseTo(850, 0);
    expect(b.w / b.h).toBeCloseTo(1365 / 2048, 3);
  });

  it("never draws bigger than the settled render", () => {
    const b = displayBox({ frameW: 4000, frameH: 3000, percent: 90, stable: stable(2048, 1365) });
    expect(b.w).toBe(2048);
  });

  it("does not apply without a measure, a settled render, or a frame that overflows", () => {
    expect(displayBox({ frameW: 0, frameH: 1000, percent: 85, stable: stable(2048, 1365) })).toBeNull();
    expect(displayBox({ frameW: 1300, frameH: 1000, percent: 85, stable: NO_STABLE_WIDTH })).toBeNull();
    expect(displayBox({ frameW: 1300, frameH: 1000, percent: 150, stable: stable(2048, 1365) })).toBeNull();
  });
});
