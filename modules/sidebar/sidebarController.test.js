import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createSidebarController } from "./sidebarController.svelte.js";

describe("sidebarController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("calculates isVisible correctly based on library root and layout", () => {
    const layouts = { cull: { sidebar: true }, dev: { sidebar: false } };
    let mode = "cull";

    const ctrl = createSidebarController({
      getLayouts: () => layouts,
      getCurrentMode: () => mode,
      getRoot: () => "/photos",
      isApplePhotosSupported: () => false,
    });

    expect(ctrl.isVisible).toBe(true);

    mode = "dev";
    expect(ctrl.isVisible).toBe(false);

    const emptyCtrl = createSidebarController({
      getLayouts: () => layouts,
      getCurrentMode: () => "cull",
      getRoot: () => null,
      isApplePhotosSupported: () => false,
    });
    expect(emptyCtrl.isVisible).toBe(false);
  });

  it("handles peek open, schedule close, and cancel", () => {
    const layouts = { cull: { sidebar: false } };
    const ctrl = createSidebarController({
      getLayouts: () => layouts,
      getCurrentMode: () => "cull",
      getRoot: () => "/photos",
      isApplePhotosSupported: () => false,
    });

    expect(ctrl.peek).toBe(false);
    ctrl.openPeek();
    expect(ctrl.peek).toBe(true);

    ctrl.schedulePeekClose();
    expect(ctrl.peek).toBe(true);

    vi.advanceTimersByTime(379);
    expect(ctrl.peek).toBe(true);

    vi.advanceTimersByTime(2);
    expect(ctrl.peek).toBe(false);

    ctrl.openPeek();
    ctrl.closePeek();
    expect(ctrl.peek).toBe(false);
  });

  it("toggles sidebar layout and triggers saveLayouts", () => {
    const layouts = { cull: { sidebar: true } };
    const saveLayouts = vi.fn();

    const ctrl = createSidebarController({
      getLayouts: () => layouts,
      getCurrentMode: () => "cull",
      getRoot: () => "/photos",
      isApplePhotosSupported: () => false,
      saveLayouts,
    });

    ctrl.toggleSidebar();
    expect(layouts.cull.sidebar).toBe(false);
    expect(saveLayouts).toHaveBeenCalled();
  });
});
