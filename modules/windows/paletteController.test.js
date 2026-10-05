import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPaletteController } from "./paletteController.js";
import * as paletteManager from "./paletteManager.js";

vi.mock("./paletteManager.js", () => ({
  syncPalettes: vi.fn(),
  syncDevPanelWindow: vi.fn(),
}));

describe("paletteController", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("emits main-dev-state when in dev mode", () => {
    const emit = vi.fn().mockReturnValue(Promise.resolve());
    const developState = {
      recipe: { exposure: 0.5 },
      developEngine: "rapid",
      renderMs: 12,
      films: [],
      papers: [],
      luts: [],
      engines: [],
      caption: "A photo",
      tags: ["tag1"],
      showClipping: false,
      checkLayer: "none",
      showCaption: false,
      developPhotoPercent: 100,
      histogram: {
        r: new Uint32Array([1, 2]),
        g: new Uint32Array([3, 4]),
        b: new Uint32Array([5, 6]),
        luma: new Uint32Array([7, 8]),
      },
    };

    const ctrl = createPaletteController({
      isTauri: true,
      emit,
      getCurrentMode: () => "dev",
      getLayouts: () => ({ dev: { devPanel: true } }),
      getSpaceLook: () => false,
      getPicked: () => "img.arw",
      getPhotoPath: () => "/photos/img.arw",
      getDevelopState: () => developState,
      getExportState: () => ({ edge: 2048, border: 0, folder: "/exports" }),
      getFrames: () => [{ path: "/photos/img.arw", rating: 4 }],
      getInstalledEditors: () => [["Photoshop", "/Applications/Photoshop.app"]],
      getStatus: () => "ready",
      saveLayouts: vi.fn(),
      switchMode: vi.fn(),
      setSpaceLook: vi.fn(),
    });

    ctrl.sendDevStateToPanel();
    expect(emit).toHaveBeenCalledWith("main-dev-state", expect.objectContaining({
      photoPath: "/photos/img.arw",
      picked: "img.arw",
      rating: 4,
      recipe: { exposure: 0.5 },
      caption: "A photo",
      histogram: {
        r: [1, 2],
        g: [3, 4],
        b: [5, 6],
        luma: [7, 8],
      },
    }));
  });

  it("coordinates toggleDevPanel with spaceLook and switchMode", () => {
    const layouts = { dev: { devPanel: true } };
    const switchMode = vi.fn();
    const setSpaceLook = vi.fn();
    const saveLayouts = vi.fn();

    let spaceLook = true;
    let mode = "cull";

    const ctrl = createPaletteController({
      isTauri: true,
      getCurrentMode: () => mode,
      getLayouts: () => layouts,
      getSpaceLook: () => spaceLook,
      getPicked: () => null,
      getPhotoPath: () => null,
      getDevelopState: () => ({}),
      getExportState: () => ({}),
      getFrames: () => [],
      getInstalledEditors: () => [],
      getStatus: () => "",
      saveLayouts,
      switchMode,
      setSpaceLook: (val) => { spaceLook = val; setSpaceLook(val); },
    });

    // SpaceLook active: turns off spaceLook and switches mode without opening dev panel
    ctrl.toggleDevPanel();
    expect(setSpaceLook).toHaveBeenCalledWith(false);
    expect(switchMode).toHaveBeenCalledWith("dev", { openDevPanel: false });

    // Mode is cull without spaceLook: switches mode to dev with openDevPanel: true
    spaceLook = false;
    ctrl.toggleDevPanel();
    expect(switchMode).toHaveBeenCalledWith("dev", { openDevPanel: true });

    // Mode is dev: toggles layouts.dev.devPanel
    mode = "dev";
    ctrl.toggleDevPanel();
    expect(layouts.dev.devPanel).toBe(false);
    expect(saveLayouts).toHaveBeenCalled();
  });
});
