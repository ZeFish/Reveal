import { describe, it, expect, vi } from "vitest";
import { createDevPanelMirror } from "./devPanelMirror.svelte.js";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((cmd) => {
    if (cmd === "default_recipe") return Promise.resolve({});
    if (cmd === "set_focus_window_presence") return Promise.resolve(null);
    return Promise.resolve(null);
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockReturnValue(Promise.resolve(vi.fn())),
  emit: vi.fn().mockReturnValue(Promise.resolve()),
}));

describe("devPanelMirror", () => {
  it("initializes develop panel mirror controller with panelProps", () => {
    const mirror = createDevPanelMirror();

    expect(mirror.panelProps).toBeDefined();
    expect(typeof mirror.toggleCheckLayer).toBe("function");
    expect(typeof mirror.toggleClipping).toBe("function");
    expect(typeof mirror.toggleCaptionOverlay).toBe("function");
    expect(typeof mirror.resetRecipe).toBe("function");
    expect(typeof mirror.hidePanel).toBe("function");
    expect(typeof mirror.engineChanged).toBe("function");
  });
});
