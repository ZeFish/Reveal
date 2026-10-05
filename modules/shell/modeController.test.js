import { describe, it, expect, vi } from "vitest";
import { createModeController } from "./modeController.svelte.js";

describe("modeController", () => {
  it("initializes with session lastMode or default cull", () => {
    const sessionMock = {
      lastMode: () => "dev",
      setLastMode: vi.fn(),
      setModeLayouts: vi.fn(),
    };
    const ctrl = createModeController({ session: sessionMock });
    expect(ctrl.currentMode).toBe("dev");
    expect(ctrl.zoomMode).toBe("frame");
    expect(ctrl.previewFilter).toBe(false);
    expect(ctrl.spaceLook).toBe(false);
    expect(ctrl.layouts.cull.sidebar).toBe(true);
  });

  it("handles mode switching to dev and back to cull", async () => {
    const sessionMock = {
      lastMode: () => "cull",
      setLastMode: vi.fn(),
      setModeLayouts: vi.fn(),
    };
    const folderSessionMock = {
      dir: "/photos/2026",
      saveMode: vi.fn(),
    };
    const closePhotoMenu = vi.fn();
    const scheduleWorkingRelease = vi.fn();
    const openPhoto = vi.fn();

    const ctrl = createModeController({
      session: sessionMock,
      getLibrary: () => ({ dir: "/photos/2026", curDir: "/photos/2026", folder: "2026" }),
      getFolderSession: () => folderSessionMock,
      closePhotoMenu,
      scheduleWorkingRelease,
      getCurrentFrame: () => ({ path: "/photos/2026/img1.raw" }),
      getPhotoPath: () => null,
      openPhoto,
    });

    await ctrl.switchMode("dev");

    expect(ctrl.currentMode).toBe("dev");
    expect(sessionMock.setLastMode).toHaveBeenCalledWith("dev");
    expect(folderSessionMock.saveMode).toHaveBeenCalledWith("dev");
    expect(openPhoto).toHaveBeenCalledWith("/photos/2026/img1.raw");

    await ctrl.switchMode("cull");
    expect(ctrl.currentMode).toBe("cull");
    expect(ctrl.zoomMode).toBe("frame");
    expect(scheduleWorkingRelease).toHaveBeenCalled();
  });

  it("handles zoom cycle only when in dev mode", () => {
    const ctrl = createModeController({});
    expect(ctrl.currentMode).toBe("cull");

    ctrl.cycleZoom();
    expect(ctrl.zoomMode).toBe("frame"); // No-op in cull

    ctrl.currentMode = "dev";
    ctrl.cycleZoom();
    expect(ctrl.zoomMode).toBe("fill");
    ctrl.cycleZoom();
    expect(ctrl.zoomMode).toBe("actual");
    ctrl.cycleZoom();
    expect(ctrl.zoomMode).toBe("frame");
  });

  it("guards editorial preview filter", () => {
    const hold = vi.fn();
    const ctrl = createModeController({
      isApplePhotosActive: () => true,
      hold,
    });

    ctrl.togglePreviewFilter();
    expect(ctrl.previewFilter).toBe(false);
    expect(hold).toHaveBeenCalledWith(
      "Editorial needs a filesystem folder. You can edit and export Apple Photos directly.",
    );

    const normalCtrl = createModeController({
      getLibrary: () => ({ curDir: "/photos", folder: "photos" }),
    });
    normalCtrl.togglePreviewFilter();
    expect(normalCtrl.previewFilter).toBe(true);
    normalCtrl.togglePreviewFilter(false);
    expect(normalCtrl.previewFilter).toBe(false);
  });

  it("toggles focus mode and calls invoke if tauri", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    const sessionMock = { setModeLayouts: vi.fn() };
    const ctrl = createModeController({
      isTauri: true,
      invoke,
      session: sessionMock,
    });

    expect(ctrl.layouts.cull.focus).toBe(false);
    await ctrl.toggleFocusMode();
    expect(ctrl.layouts.cull.focus).toBe(true);
    expect(invoke).toHaveBeenCalledWith("set_focus", { enabled: true });
    expect(sessionMock.setModeLayouts).toHaveBeenCalled();
  });
});
