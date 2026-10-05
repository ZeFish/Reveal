import { describe, it, expect, vi } from "vitest";
import { nextZoomMode, createPanGesture } from "./surfaceController.js";

describe("surfaceController", () => {
  it("cycles through zoom modes", () => {
    expect(nextZoomMode("frame")).toBe("fill");
    expect(nextZoomMode("fill")).toBe("actual");
    expect(nextZoomMode("actual")).toBe("frame");

    expect(nextZoomMode("frame", true)).toBe("actual");
    expect(nextZoomMode("actual", true)).toBe("fill");
  });

  it("handles pan gesture lifecycle", () => {
    const onPanningChange = vi.fn();
    const pan = createPanGesture({
      getZoomMode: () => "actual",
      getPhotoPercent: () => 100,
      onPanningChange,
    });

    const el = {
      scrollLeft: 10,
      scrollTop: 20,
      setPointerCapture: vi.fn(),
      releasePointerCapture: vi.fn(),
    };

    const downEvent = {
      button: 0,
      clientX: 50,
      clientY: 60,
      pointerId: 1,
      currentTarget: el,
      stopPropagation: vi.fn(),
    };

    pan.onPointerDown(downEvent);
    expect(pan.isPanning()).toBe(true);
    expect(onPanningChange).toHaveBeenCalledWith(true);
    expect(el.setPointerCapture).toHaveBeenCalledWith(1);

    const moveEvent = {
      clientX: 40,
      clientY: 50,
      currentTarget: el,
    };
    pan.onPointerMove(moveEvent);
    expect(el.scrollLeft).toBe(20); // 10 - (40 - 50) = 20
    expect(el.scrollTop).toBe(30);

    const upEvent = {
      pointerId: 1,
      currentTarget: el,
    };
    pan.onPointerUp(upEvent);
    expect(pan.isPanning()).toBe(false);
    expect(onPanningChange).toHaveBeenCalledWith(false);
    expect(el.releasePointerCapture).toHaveBeenCalledWith(1);
  });
});
