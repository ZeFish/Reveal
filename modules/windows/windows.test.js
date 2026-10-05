import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  paletteTitle,
  PALETTE_SPECS,
} from "./paletteManager.js";
import {
  fullscreenState,
  enterFullscreen,
  exitFullscreen,
  toggleFullscreen,
} from "./fullscreenState.svelte.js";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: {
    getByLabel: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    close: vi.fn(),
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    startDragging: vi.fn(),
    setFocus: vi.fn(),
  })),
  currentMonitor: vi.fn(),
  availableMonitors: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("modules/windows", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    fullscreenState.active = false;
    fullscreenState.url = null;
    fullscreenState.requestId = 0;
  });

  describe("paletteManager", () => {
    it("generates correct palette titles", () => {
      expect(paletteTitle("develop-panel", "DSC_0001.ARW")).toBe("DSC_0001.ARW — Darkroom");
      expect(paletteTitle("develop-panel", null)).toBe("Darkroom");
      expect(paletteTitle("lut-panel")).toBe("LUTs");
      expect(paletteTitle("preset-panel")).toBe("Presets");
      expect(paletteTitle("other")).toBe("Reveal");
    });

    it("has develop-panel in PALETTE_SPECS", () => {
      expect(PALETTE_SPECS.some((p) => p.label === "develop-panel")).toBe(true);
    });
  });

  describe("fullscreenState", () => {
    it("enters and exits fullscreen", async () => {
      const frame = { path: "/photos/a.jpg" };
      const prepareFrame = vi.fn();

      await enterFullscreen({ frame, prepareFrame });

      expect(prepareFrame).toHaveBeenCalledWith("/photos/a.jpg");
      expect(fullscreenState.active).toBe(true);

      await exitFullscreen();

      expect(fullscreenState.active).toBe(false);
      expect(fullscreenState.url).toBeNull();
    });

    it("toggles fullscreen mode", async () => {
      const frame = { path: "/photos/b.jpg" };

      expect(fullscreenState.active).toBe(false);

      toggleFullscreen({ frame });
      expect(fullscreenState.active).toBe(true);

      toggleFullscreen({});
      expect(fullscreenState.active).toBe(false);
    });

    it("operates through createFullscreenController", async () => {
      const { createFullscreenController } = await import("./fullscreenController.js");
      const frames = [{ path: "/photos/c.jpg" }];
      const previewUrl = vi.fn((path) => `preview://${path}`);
      const onSyncDevPanel = vi.fn();

      const ctrl = createFullscreenController({
        getLibraryFrames: () => frames,
        getCurrentMode: () => "cull",
        getPhotoPath: () => null,
        getImgUrl: () => null,
        previewUrl,
        getView: () => frames,
        getSel: () => 0,
        getLayouts: () => ({ dev: { devPanel: false } }),
        onSyncDevPanel,
      });

      await ctrl.enterFullscreen();
      expect(fullscreenState.active).toBe(true);

      await ctrl.exitFullscreen();
      expect(fullscreenState.active).toBe(false);

      await ctrl.toggleFullscreen();
      expect(fullscreenState.active).toBe(true);

      await ctrl.toggleFullscreen();
      expect(fullscreenState.active).toBe(false);
    });
  });

  describe("windowControls extras", () => {
    it("calls toggle_system_appearance via toggleAppearance", async () => {
      const { toggleAppearance, hideWindow } = await import("./windowControls.js");
      const invokeMock = vi.fn().mockResolvedValue(null);
      const notify = vi.fn();

      await toggleAppearance({ invoke: invokeMock, notify });
      expect(invokeMock).toHaveBeenCalledWith("toggle_system_appearance");

      await hideWindow({ invoke: invokeMock });
      expect(invokeMock).toHaveBeenCalledWith("hide_contact_sheet");
    });
  });
});

