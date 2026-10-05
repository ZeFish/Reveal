import { describe, it, expect, vi } from "vitest";
import { createAppEventHandlers } from "./appEventHandlers.js";

describe("createAppEventHandlers", () => {
  it("creates fully-configured handlers bag for app events, devPanel and menu bridges", async () => {
    const notify = vi.fn();
    const saveLayouts = vi.fn();
    const setGardenAccount = vi.fn();
    const layouts = { cull: { focus: false }, dev: { focus: false, devPanel: true } };
    const importState = { autoImport: false, importDir: null, cards: [] };

    const handlers = createAppEventHandlers({
      session: {},
      layouts,
      saveLayouts,
      getCurrentMode: () => "cull",
      switchMode: vi.fn(),
      importState,
      libraryState: {},
      library: { curDir: "/photos", frames: [] },
      cullingState: { minRating: 0 },
      developState: { recipe: null },
      exportState: {},
      saveExportPrefs: vi.fn(),
      refreshDirs: vi.fn().mockResolvedValue(true),
      openDir: vi.fn(),
      openPhoto: vi.fn(),
      invoke: vi.fn(),
      notify,
      setProgress: vi.fn(),
      updateActivity: vi.fn(),
      activity: {},
      triggerAiCull: vi.fn(),
      pollCards: vi.fn().mockResolvedValue([]),
      importCard: vi.fn(),
      handleCardMounted: vi.fn(),
      handleCardUnmounted: vi.fn(),
      handleCullStarted: vi.fn(),
      handleCullProgress: vi.fn(),
      handleCullFinished: vi.fn(),
      handleCullFailed: vi.fn(),
      refreshLoadedFrames: vi.fn(),
      getSourceOffline: () => false,
      setSourceOffline: vi.fn(),
      getPhotoPath: () => null,
      getCurrentFramePath: () => undefined,
      setGardenAccount,
      sendDevStateToPanel: vi.fn(),
      sendSettingsToPanel: vi.fn(),
      choosePreferenceFolder: vi.fn(),
      saveSettingsFromPanel: vi.fn(),
      openSettings: vi.fn(),
      edited: vi.fn(),
      captionEdited: vi.fn(),
      tagsEdited: vi.fn(),
      applyEngineChange: vi.fn(),
      applyResetRecipe: vi.fn(),
      toggleCheckLayer: vi.fn(),
      clearDevelopment: vi.fn(),
      scheduleRender: vi.fn(),
      exportCurrent: vi.fn(),
      exportSelection: vi.fn(),
      exportToDailyNote: vi.fn(),
      chooseExportFolder: vi.fn(),
      openInEditor: vi.fn(),
      onKey: vi.fn(),
      cycleZoom: vi.fn(),
      togglePresetPanel: vi.fn(),
      toggleLutPanel: vi.fn(),
      toggleDevPanel: vi.fn(),
      selectedFrames: () => [],
      getView: () => [],
      getSel: () => 0,
      applyRecipeToFrames: vi.fn(),
      setQueueOpen: vi.fn(),
      modalState: { toggleShortcuts: vi.fn() },
      preferences: { obsidian_enabled: true },
      developFromMenu: vi.fn(),
      toggleAutoImport: vi.fn(),
      indexRoot: vi.fn(),
    });

    // Test garden account changed
    handlers.onGardenAccountChanged({ signed_in: true, username: "francis" });
    expect(setGardenAccount).toHaveBeenCalledWith({ signed_in: true, username: "francis" });

    // Test focus mode changed
    handlers.onFocusModeChanged({ enabled: true });
    expect(layouts.cull.focus).toBe(true);
    expect(saveLayouts).toHaveBeenCalled();

    // Test shell prefs changed
    handlers.onShellPrefsChanged({ auto_import: true, import_dir: "/imports", focus_mode: false });
    expect(importState.autoImport).toBe(true);
    expect(importState.importDir).toBe("/imports");
    expect(layouts.cull.focus).toBe(false);

    // Test app error
    handlers.onAppError(new Error("Test crash"));
    expect(notify).toHaveBeenCalledWith("Test crash", 5000);
  });
});
