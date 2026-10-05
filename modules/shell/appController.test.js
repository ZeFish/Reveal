import { describe, it, expect, vi, beforeEach } from "vitest";
import { createAppController } from "./appController.svelte.js";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((cmd) => {
    if (cmd === "load_shell_prefs") return Promise.resolve({});
    if (cmd === "load_preferences") return Promise.resolve({});
    if (cmd === "list_profiles") return Promise.resolve([]);
    if (cmd === "list_luts") return Promise.resolve([]);
    if (cmd === "list_engines") return Promise.resolve([]);
    return Promise.resolve(null);
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockReturnValue(Promise.resolve(vi.fn())),
  emit: vi.fn().mockReturnValue(Promise.resolve()),
}));

describe("appController", () => {
  it("creates controller with initial properties and workspace props", () => {
    const app = createAppController();

    expect(app.currentMode).toBe("cull");
    expect(app.cullProps).toBeDefined();
    expect(app.developProps).toBeDefined();
    expect(typeof app.onKey).toBe("function");
    expect(typeof app.closeMainWindow).toBe("function");
    expect(typeof app.minimizeMainWindow).toBe("function");
    expect(typeof app.zoomMainWindow).toBe("function");
    expect(typeof app.exitFullscreen).toBe("function");
    expect(app.currentScrollTop).toBe(0);
    app.currentScrollTop = 150;
    expect(app.currentScrollTop).toBe(150);
    expect(app.applePhotos).toBeDefined();
    expect(typeof app.cancelApplePhotosTransfer).toBe("function");
    expect(typeof app.dismissApplePhotosTransfer).toBe("function");
  });
});
