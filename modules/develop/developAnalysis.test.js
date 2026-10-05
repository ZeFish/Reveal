import { describe, it, expect, vi } from "vitest";
import { createScopeAnalyzer, renderCheckLayer, WF_COLS, WF_LEVELS, VEC_SIZE } from "./developAnalysis.js";

describe("developAnalysis", () => {
  it("computes histogram and scopes correctly from synthetic pixel buffer", () => {
    const analyzer = createScopeAnalyzer();
    const width = 4;
    const height = 4;
    const data = new Uint8ClampedArray(width * height * 4);

    // Fill with semi-transparent, mid-gray and saturated red pixels
    for (let i = 0; i < data.length; i += 4) {
      data[i] = 128;     // R
      data[i + 1] = 64;  // G
      data[i + 2] = 32;  // B
      data[i + 3] = 255; // A
    }

    const result = analyzer.analyze({ width, height, data });

    expect(result.histogram).toBeDefined();
    expect(result.histogram.r[128]).toBe(16);
    expect(result.histogram.g[64]).toBe(16);
    expect(result.histogram.b[32]).toBe(16);
    expect(result.scopes.r.length).toBe(WF_COLS * WF_LEVELS);
    expect(result.scopes.vector.length).toBe(VEC_SIZE * VEC_SIZE);
  });

  it("renders clipping check layer without error", () => {
    const width = 2;
    const height = 2;
    const data = new Uint8ClampedArray([
      255, 255, 255, 255, // clipped white
      0, 0, 0, 255,       // clipped black
      128, 128, 128, 255, // mid
      100, 100, 100, 0,   // transparent
    ]);

    const sourceData = { data };
    const putImageData = vi.fn();
    const createImageData = vi.fn().mockReturnValue({ data: new Uint8ClampedArray(width * height * 4) });
    const ctx = /** @type {any} */ ({ putImageData, createImageData });

    renderCheckLayer(/** @type {any} */ (sourceData), "clipping", ctx, width, height);

    expect(createImageData).toHaveBeenCalledWith(width, height);
    expect(putImageData).toHaveBeenCalled();
  });
});
