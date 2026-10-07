import { describe, it, expect } from "vitest";
import { visiblePaths } from "./visibleCells.js";

const frames = Array.from({ length: 40 }, (_, i) => ({ path: `/p${i}.raw` }));

describe("visiblePaths", () => {
  it("is the rows in the viewport, whole rows, no buffer", () => {
    // rows of 4, 100px pitch, 250px tall viewport at the top: rows 0 to 3 touch it
    const paths = visiblePaths(frames, { top: 0, viewH: 250, rowPitch: 100, cols: 4 });
    expect(paths[0]).toBe("/p0.raw");
    expect(paths.length).toBe(16);
  });

  it("follows the scroll", () => {
    const paths = visiblePaths(frames, { top: 300, viewH: 100, rowPitch: 100, cols: 4 });
    expect(paths[0]).toBe("/p12.raw");
  });

  it("is empty until the grid has a size", () => {
    expect(visiblePaths(frames, { top: 0, viewH: 0, rowPitch: 100, cols: 4 })).toEqual([]);
    expect(visiblePaths(frames, { top: 0, viewH: 500, rowPitch: 0, cols: 4 })).toEqual([]);
  });
});
