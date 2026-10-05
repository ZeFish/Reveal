import { describe, it, expect, vi, beforeEach } from "vitest";
import { startAppLifecycle } from "./appWiring.js";

describe("startAppLifecycle", () => {
  let mockOptions;

  beforeEach(() => {
    mockOptions = {
      isTauri: true,
      listen: vi.fn().mockReturnValue(Promise.resolve(vi.fn())),
      invoke: vi.fn().mockImplementation((cmd) => {
        if (cmd === "load_shell_prefs") return Promise.resolve({ auto_import: false });
        if (cmd === "load_preferences") return Promise.resolve({});
        if (cmd === "list_profiles") return Promise.resolve([]);
        if (cmd === "list_luts") return Promise.resolve([]);
        if (cmd === "list_engines") return Promise.resolve([]);
        return Promise.resolve(null);
      }),
      session: {
        modeLayouts: vi.fn(),
        gridPrefs: vi.fn(),
        gridLayout: vi.fn(),
        exportPrefs: vi.fn(),
        lastDir: vi.fn().mockReturnValue(null),
        lastPhoto: vi.fn().mockReturnValue(null),
      },
      layouts: { dev: {} },
      saveLayouts: vi.fn(),
      modeCtrl: { currentMode: "cull", cycleZoom: vi.fn() },
      importCtrl: {
        pollCards: vi.fn().mockResolvedValue([]),
        importCard: vi.fn(),
        handleCardMounted: vi.fn(),
        handleCardUnmounted: vi.fn(),
        toggleAutoImport: vi.fn(),
      },
      libraryController: {
        refreshLoadedFrames: vi.fn(),
        indexRoot: vi.fn(),
      },
      navigationCtrl: {
        openDir: vi.fn(),
        openFolder: vi.fn(),
        openPhoto: vi.fn(),
      },
      cullingCtrl: {
        triggerAiCull: vi.fn(),
      },
      exportCtrl: {
        saveExportPrefs: vi.fn(),
        exportCurrent: vi.fn(),
        exportSelection: vi.fn(),
        exportToDailyNote: vi.fn(),
        chooseExportFolder: vi.fn(),
      },
      settingsCtrl: {
        preferences: {},
        sendSettingsToPanel: vi.fn(),
        choosePreferenceFolder: vi.fn(),
        saveSettingsFromPanel: vi.fn(),
        openSettings: vi.fn(),
      },
      photoMenuCtrl: {
        developFromMenu: vi.fn(),
      },
      externalEditorsCtrl: {
        loadExternalEditors: vi.fn().mockResolvedValue([]),
        openInEditor: vi.fn(),
      },
      devController: {
        edited: vi.fn(),
        captionEdited: vi.fn(),
        tagsEdited: vi.fn(),
        applyEngineChange: vi.fn(),
        applyResetRecipe: vi.fn(),
        toggleCheckLayer: vi.fn(),
        clearDevelopment: vi.fn(),
        scheduleRender: vi.fn(),
        applyRecipeToFrames: vi.fn(),
      },
      keyboardCtrl: {
        onKey: vi.fn(),
      },
      paletteCtrl: {
        sendDevStateToPanel: vi.fn(),
        toggleDevPanel: vi.fn(),
      },
      modalState: {},
      libraryState: {},
      library: { dir: "/photos", dirs: [], frames: [] },
      cullingState: {},
      developState: {},
      exportState: {},
      importState: {},
      settingsState: { set: vi.fn() },
      initCullingState: vi.fn(),
      loadPreferences: vi.fn().mockResolvedValue({}),
      getCurrentMode: () => "cull",
      switchMode: vi.fn(),
      refreshDirs: vi.fn().mockResolvedValue([]),
      rescan: vi.fn(),
      getPublishTaskId: () => null,
      getSourceOffline: () => false,
      setSourceOffline: vi.fn(),
      getPhotoPath: () => null,
      getCurrentFramePath: () => undefined,
      getView: () => [],
      getSel: () => 0,
      selectedFrames: () => [],
      setGpuAvailable: vi.fn(),
      setGardenAccount: vi.fn(),
    };
  });

  it("returns no-op cleanup when not in Tauri", () => {
    mockOptions.isTauri = false;
    const cleanup = startAppLifecycle(mockOptions);
    expect(typeof cleanup).toBe("function");
    cleanup();
    expect(mockOptions.listen).not.toHaveBeenCalled();
  });

  it("initializes session and registers listeners in Tauri", async () => {
    vi.useFakeTimers();
    const cleanup = startAppLifecycle(mockOptions);

    expect(mockOptions.session.modeLayouts).toHaveBeenCalled();
    expect(mockOptions.initCullingState).toHaveBeenCalled();

    // Fast-forward microtasks
    await vi.advanceTimersByTimeAsync(10);

    expect(mockOptions.externalEditorsCtrl.loadExternalEditors).toHaveBeenCalled();
    expect(mockOptions.listen).toHaveBeenCalled();
    expect(mockOptions.importCtrl.pollCards).toHaveBeenCalled();

    cleanup();
    vi.useRealTimers();
  });
});
