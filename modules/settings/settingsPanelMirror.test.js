import { describe, it, expect, vi } from "vitest";
import { createSettingsPanelMirror } from "./settingsPanelMirror.svelte.js";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((cmd) => {
    if (cmd === "load_shell_prefs") return Promise.resolve({});
    if (cmd === "load_preferences") return Promise.resolve({});
    if (cmd === "catalog_roots") return Promise.resolve([]);
    if (cmd === "list_engines") return Promise.resolve([]);
    if (cmd === "list_presets") return Promise.resolve([]);
    return Promise.resolve(null);
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockReturnValue(Promise.resolve(vi.fn())),
  emit: vi.fn().mockReturnValue(Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn().mockReturnValue({
    close: vi.fn(),
    scaleFactor: vi.fn().mockResolvedValue(2),
    innerSize: vi.fn().mockResolvedValue({ width: 1000, height: 800 }),
    setSize: vi.fn().mockResolvedValue(undefined),
  }),
}));

describe("settingsPanelMirror", () => {
  it("initializes with default preferences and exposes panelProps", () => {
    const mirror = createSettingsPanelMirror();

    expect(mirror.preferences).toBeDefined();
    expect(mirror.panelProps).toBeDefined();
    expect(typeof mirror.close).toBe("function");
    expect(typeof mirror.save).toBe("function");
    expect(typeof mirror.chooseFolder).toBe("function");
    expect(typeof mirror.selectTheme).toBe("function");
  });
});
