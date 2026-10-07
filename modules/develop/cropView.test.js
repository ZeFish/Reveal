import { describe, it, expect } from "vitest";
import { cropView, dragPicture, CROP_STAGE_FILL } from "./cropView.js";

const stage = { w: 1200, h: 800 };
const photo = { x: 100, y: 100, w: 1000, h: 600 }; // centred: (100+500, 100+300) = (600, 400)

/** Where the transform puts a point of the photo, relative to the photo's centre. */
const screenOf = (v, qx, qy, { flipH = 1, flipV = 1 } = {}) => ({
  x: photo.x + photo.w / 2 + v.tx + v.s * qx * flipH,
  y: photo.y + photo.h / 2 + v.ty + v.s * qy * flipV,
});

describe("cropView", () => {
  it("puts the centre of the crop on the centre of the stage, wherever the crop is", () => {
    for (const crop of [
      { x: 0, y: 0, w: 1, h: 1 },
      { x: 0.1, y: 0.2, w: 0.3, h: 0.4 },
      { x: 0.6, y: 0.55, w: 0.4, h: 0.45 },
    ]) {
      const v = cropView({ stage, photo, crop });
      const qc = screenOf(v, (crop.x + crop.w / 2 - 0.5) * photo.w, (crop.y + crop.h / 2 - 0.5) * photo.h);
      expect(qc.x).toBeCloseTo(stage.w / 2, 4);
      expect(qc.y).toBeCloseTo(stage.h / 2, 4);
    }
  });

  it("fills the stage with the crop (the limiting side), leaving the air around it", () => {
    const crop = { x: 0.25, y: 0.25, w: 0.5, h: 0.5 }; // 500 x 300 layout px
    const v = cropView({ stage, photo, crop });
    expect(v.s).toBeCloseTo(Math.min((1200 * CROP_STAGE_FILL) / 500, (800 * CROP_STAGE_FILL) / 300), 6);
    expect(500 * v.s).toBeLessThanOrEqual(1200 * CROP_STAGE_FILL + 1e-6);
    expect(300 * v.s).toBeLessThanOrEqual(800 * CROP_STAGE_FILL + 1e-6);
  });

  it("keeps the same centring when the picture is flipped", () => {
    const crop = { x: 0.1, y: 0.15, w: 0.35, h: 0.4 };
    const qx = (crop.x + crop.w / 2 - 0.5) * photo.w;
    const qy = (crop.y + crop.h / 2 - 0.5) * photo.h;
    for (const opts of [{ flipH: -1 }, { flipV: -1 }, { flipH: -1, flipV: -1 }]) {
      const v = cropView({ stage, photo, crop, ...opts });
      const p = screenOf(v, qx, qy, opts);
      expect(p.x).toBeCloseTo(stage.w / 2, 4);
      expect(p.y).toBeCloseTo(stage.h / 2, 4);
    }
  });

  it("does not move or zoom the frame when the picture is straightened: only the picture turns", () => {
    const crop = { x: 0.1, y: 0.15, w: 0.35, h: 0.4 };
    // `angle` is not an input any more; passing one changes nothing.
    expect(cropView({ stage, photo, crop, angle: 12 })).toEqual(cropView({ stage, photo, crop }));
  });

  it("holds the zoom still when it is pinned (while the picture is dragged)", () => {
    const a = cropView({ stage, photo, crop: { x: 0.1, y: 0.1, w: 0.4, h: 0.4 } });
    const b = cropView({ stage, photo, crop: { x: 0.3, y: 0.4, w: 0.4, h: 0.4 }, scale: a.s });
    expect(b.s).toBe(a.s);
    expect(b.tx).not.toBe(a.tx);
  });

  it("does not zoom without limit on a tiny crop, and says nothing before it is measured", () => {
    expect(cropView({ stage, photo, crop: { x: 0, y: 0, w: 0.001, h: 0.001 } }).s).toBe(8);
    expect(cropView({ stage: { w: 0, h: 0 }, photo, crop: { x: 0, y: 0, w: 1, h: 1 } })).toBeNull();
  });
});

describe("dragPicture", () => {
  const crop = { x: 0.3, y: 0.3, w: 0.4, h: 0.4 };
  it("moves the crop the opposite way to the picture", () => {
    const next = dragPicture(crop, 0.1, -0.05);
    expect(next.x).toBeCloseTo(0.2);
    expect(next.y).toBeCloseTo(0.35);
    expect(next.w).toBe(0.4);
  });
  it("stops at the photo's edge", () => {
    expect(dragPicture(crop, -2, 2)).toMatchObject({ x: 0.6, y: 0 });
    expect(dragPicture(crop, 2, -2)).toMatchObject({ x: 0, y: 0.6 });
  });
});
