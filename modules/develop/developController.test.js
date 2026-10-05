import { describe, it, expect, vi } from "vitest";
import { createDevelopController } from "./developController.js";

describe("createDevelopController", () => {
  it("coordinates recipe editing, lut manipulation and docking actions", async () => {
    const state = {
      recipe: { exposure: 0, rapid_pre_luts: [] },
      developEngine: "rapid",
      showCaption: false,
      lastEditedKey: null,
    };
    const scheduleRender = vi.fn();
    const invoke = vi.fn().mockResolvedValue({});
    const layouts = { dev: { devPanel: true } };
    const saveLayouts = vi.fn();
    const saveExportPrefs = vi.fn();

    const controller = createDevelopController({
      state,
      getPhotoPath: () => "/photos/test.arw",
      getCurrentMode: () => "dev",
      liveRenderPx: () => 1024,
      scheduleRender,
      invoke,
      layouts,
      saveLayouts,
      saveExportPrefs,
    });

    // Test setDevNum
    controller.setDevNum("exposure", 0.5, false);
    expect(state.recipe.exposure).toBe(0.5);
    expect(state.lastEditedKey).toBe("exposure");

    // Test caption overlay toggle
    controller.dockedToggleCaptionOverlay();
    expect(state.showCaption).toBe(true);

    // Test hide panel
    controller.dockedHidePanel();
    expect(layouts.dev.devPanel).toBe(false);
    expect(saveLayouts).toHaveBeenCalled();

    // Test export prefs
    controller.dockedExportSettingsChanged();
    expect(saveExportPrefs).toHaveBeenCalled();
  });

  it("coordinates copySettings, applyRecipeToFrames, and pasteSettings", async () => {
    const state = {
      recipe: { exposure: 1 },
      copiedRecipe: null,
    };
    const invoke = vi.fn().mockImplementation(async (cmd) => {
      if (cmd === "load_sidecar") return { engine_settings: { exposure: 0.8 } };
      return {};
    });
    const notify = vi.fn();
    const targetFrames = [{ path: "/photos/p1.jpg" }, { path: "/photos/p2.jpg" }];

    const controller = createDevelopController({
      state,
      getPhotoPath: () => "/photos/p1.jpg",
      getCurrentMode: () => "dev",
      liveRenderPx: () => 1024,
      scheduleRender: vi.fn(),
      invoke,
      notify,
      getSourceFrame: () => ({ path: "/photos/p1.jpg" }),
      getSelectedFrames: () => targetFrames,
      freshPreviewVersion: vi.fn().mockResolvedValue(1),
      refreshFrames: vi.fn(),
    });

    await controller.copySettings();
    expect(state.copiedRecipe).toEqual({ exposure: 1 });
    expect(notify).toHaveBeenCalledWith(expect.stringContaining("Settings copied"), expect.any(Number));

    // Test pasteSettings
    await controller.pasteSettings();
    expect(invoke).toHaveBeenCalledWith("save_recipe", expect.objectContaining({
      path: "/photos/p1.jpg",
      recipe: expect.objectContaining({ exposure: 1 }),
    }));
  });
});
