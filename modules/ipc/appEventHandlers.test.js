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
  /** The few dependencies the import-finished path reads. */
  function importDeps(overrides = {}) {
    return {
      setProgress: vi.fn(),
      notify: vi.fn(),
      importState: { importedByFolder: new Map() },
      cullingState: {},
      refreshDirs: vi.fn().mockResolvedValue(true),
      openDir: vi.fn().mockResolvedValue(undefined),
      triggerAiCull: vi.fn(),
      ...overrides,
    };
  }

  it("opens the most recent imported day when an import finishes", async () => {
    const deps = importDeps();
    const handlers = createAppEventHandlers(deps);
    await handlers.onImportFinished({ copied: 3, skipped: 0, folders: ["/a/2026-10-05", "/a/2026-10-06"] });
    expect(deps.openDir).toHaveBeenCalledWith("/a/2026-10-06");
    expect(deps.importState.lastImportedFolder).toBe("/a/2026-10-06");
  });

  it("still opens the folder when the folder tree refresh fails or hangs", async () => {
    const failing = importDeps({ refreshDirs: vi.fn().mockRejectedValue(new Error("nas slow")) });
    await createAppEventHandlers(failing).onImportFinished({ folders: ["/a/2026-10-06"] });
    expect(failing.openDir).toHaveBeenCalledWith("/a/2026-10-06");

    // The full refresh (not the light one) never settles: navigation does not wait for it.
    const hanging = importDeps({
      refreshDirs: vi.fn((light) => (light ? Promise.resolve(true) : new Promise(() => {}))),
    });
    await createAppEventHandlers(hanging).onImportFinished({ folders: ["/a/2026-10-06"] });
    expect(hanging.openDir).toHaveBeenCalledWith("/a/2026-10-06");
    expect(hanging.refreshDirs).toHaveBeenCalledWith(true);
  });

  it("does not move when the card brought no folder at all", async () => {
    const deps = importDeps();
    await createAppEventHandlers(deps).onImportFinished({ copied: 0, skipped: 0, folders: [] });
    expect(deps.openDir).not.toHaveBeenCalled();
  });
  it("runs the AI cull on the new photos even when opening the folder never settles", async () => {
    const triggerAiCull = vi.fn().mockResolvedValue(undefined);
    const importedByFolder = new Map([
      ["/a/2026-10-06", ["/a/2026-10-06/x.raf", "/a/2026-10-06/y.raf"]],
    ]);
    const deps = importDeps({
      importState: { importedByFolder },
      cullingState: { aiCullMarkStory: true, aiCullExportDesktop: false },
      refreshDirs: vi.fn(() => new Promise(() => {})), // the tree never answers
      openDir: vi.fn(() => new Promise(() => {})),
      triggerAiCull,
    });
    // onImportFinished itself waits on the stalled tree; the cull must already be under way.
    createAppEventHandlers(deps).onImportFinished({ copied: 2, skipped: 1, folders: ["/a/2026-10-05", "/a/2026-10-06"] });
    await new Promise((r) => setTimeout(r, 0));
    expect(triggerAiCull).toHaveBeenCalledTimes(1);
    expect(triggerAiCull).toHaveBeenCalledWith("/a/2026-10-06", ["/a/2026-10-06/x.raf", "/a/2026-10-06/y.raf"]);
  });

  it("does not cull when the AI options are off", async () => {
    const deps = importDeps({
      cullingState: { aiCullMarkStory: false, aiCullExportDesktop: false },
      importState: { importedByFolder: new Map([["/a/2026-10-06", ["/a/2026-10-06/x.raf"]]]) },
    });
    await createAppEventHandlers(deps).onImportFinished({ folders: ["/a/2026-10-06"] });
    expect(deps.triggerAiCull).not.toHaveBeenCalled();
  });
  it("gives a frame the new preview version when the sidecar has just been rewritten", () => {
    const frame = { path: "/a/x.raf", previewVersion: 5 };
    const other = { path: "/a/y.raf", previewVersion: 9 };
    const refreshLoadedFrames = vi.fn();
    const deps = importDeps({ library: { frames: [frame, other] }, refreshLoadedFrames });
    const handlers = createAppEventHandlers(deps);
    handlers.onPreviewPublished({ path: "/a/x.raf", version: 7 });
    expect(frame.previewVersion).toBe(7);
    expect(other.previewVersion).toBe(9);
    expect(refreshLoadedFrames).toHaveBeenCalledTimes(1);
    // The same version again, or a photo that is not on screen: nothing to redraw.
    handlers.onPreviewPublished({ path: "/a/x.raf", version: 7 });
    handlers.onPreviewPublished({ path: "/elsewhere.raf", version: 1 });
    expect(refreshLoadedFrames).toHaveBeenCalledTimes(1);
  });

  it("shows the photos an import has copied into the folder on screen, without waiting for the end", async () => {
    vi.useFakeTimers();
    try {
      const rows = [{ name: "a.RAF", path: "/in/a.RAF" }, { name: "b.RAF", path: "/in/b.RAF" }, { name: ".hidden", path: "/in/.hidden" }];
      const refreshLoadedFrames = vi.fn();
      const refreshDirs = vi.fn().mockResolvedValue(true);
      const deps = importDeps({
        library: { curDir: "/in", frames: [] },
        cullingState: { minRating: 0 },
        invoke: vi.fn().mockResolvedValue(rows),
        refreshLoadedFrames,
        refreshDirs,
      });
      const handlers = createAppEventHandlers(deps);
      // A burst: the last event must not be dropped just because it came inside the window.
      await handlers.onImportProgress({ done: 1, total: 3, destDir: "/in", dest: "/in/a.RAF" });
      await handlers.onImportProgress({ done: 2, total: 3, destDir: "/in", dest: "/in/b.RAF" });
      await handlers.onImportProgress({ done: 3, total: 3, destDir: "/in", dest: "/in/c.RAF" });
      await vi.advanceTimersByTimeAsync(300);
      expect(deps.invoke).toHaveBeenCalledWith("index_frames", { dir: "/in", minRating: 0 });
      // Hidden files stay out, and the photos are shown before the folder tree has finished.
      expect(refreshLoadedFrames).toHaveBeenCalledWith(rows.slice(0, 2));
      expect(refreshLoadedFrames.mock.invocationCallOrder[0]).toBeLessThan(refreshDirs.mock.invocationCallOrder[0]);
      // Another folder is not the one on screen: nothing is replaced there.
      refreshLoadedFrames.mockClear();
      await handlers.onImportProgress({ done: 3, total: 3, destDir: "/elsewhere", dest: "/elsewhere/z.RAF" });
      await vi.advanceTimersByTimeAsync(300);
      expect(refreshLoadedFrames).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not start the AI pass or the desktop export on an import the person stopped", async () => {
    const deps = importDeps({
      cullingState: { aiCullMarkStory: true, aiCullExportDesktop: true },
      importState: { importedByFolder: new Map([["/a/2026-10-06", ["/a/2026-10-06/x.RAF"]]]) },
    });
    const handlers = createAppEventHandlers(deps);
    await handlers.onImportFinished({ copied: 31, skipped: 0, cancelled: true, folders: ["/a/2026-10-06"] });
    expect(deps.triggerAiCull).not.toHaveBeenCalled();
    expect(deps.notify).toHaveBeenCalledWith("Import stopped", 4000);
    // The photos already copied are still shown.
    expect(deps.openDir).toHaveBeenCalledWith("/a/2026-10-06");
  });

  it("does not start the AI pass on an import where files failed to copy, such as a card pulled out", async () => {
    const deps = importDeps({
      cullingState: { aiCullMarkStory: true, aiCullExportDesktop: true },
      importState: { importedByFolder: new Map([["/a/2026-10-06", ["/a/2026-10-06/x.RAF"]]]) },
    });
    await createAppEventHandlers(deps).onImportFinished({ copied: 31, failed: 3, folders: ["/a/2026-10-06"] });
    expect(deps.triggerAiCull).not.toHaveBeenCalled();
    expect(deps.notify).toHaveBeenCalledWith("Import incomplete — 3 photos could not be copied", 8000);
    expect(deps.openDir).toHaveBeenCalledWith("/a/2026-10-06");
  });

  it("still runs the AI pass on an import that succeeded", async () => {
    const deps = importDeps({
      cullingState: { aiCullMarkStory: true },
      importState: { importedByFolder: new Map([["/a/2026-10-06", ["/a/2026-10-06/x.RAF"]]]) },
    });
    await createAppEventHandlers(deps).onImportFinished({ copied: 31, failed: 0, cancelled: false, folders: ["/a/2026-10-06"] });
    await new Promise((r) => setTimeout(r, 0));
    expect(deps.triggerAiCull).toHaveBeenCalledWith("/a/2026-10-06", ["/a/2026-10-06/x.RAF"]);
  });

  it("gives a photo a new preview version when its develop settings have been cleared from disk", () => {
    const frame = { path: "/a/x.raf", previewVersion: 5 };
    const other = { path: "/a/y.raf", previewVersion: 9 };
    const refreshLoadedFrames = vi.fn();
    const deps = importDeps({ library: { frames: [frame, other] }, refreshLoadedFrames });
    const handlers = createAppEventHandlers(deps);
    handlers.onPreviewCleared({ path: "/a/x.raf" });
    expect(frame.previewVersion).not.toBe(5);
    expect(other.previewVersion).toBe(9);
    expect(refreshLoadedFrames).toHaveBeenCalledTimes(1);
    handlers.onPreviewCleared({ path: "/elsewhere.raf" }); // not on screen: nothing to redraw
    expect(refreshLoadedFrames).toHaveBeenCalledTimes(1);
  });
});
