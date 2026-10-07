import { describe, it, expect, vi } from "vitest";
import { initSessionBoot, performAppBoot } from "./appBoot.js";

describe("appBoot", () => {
  describe("initSessionBoot", () => {
    it("restores mode layouts, culling prefs, and export prefs from session", () => {
      const layouts = { cull: { sidebar: true } };
      const session = {
        modeLayouts: () => ({ cull: { sidebar: false }, dev: { sidebar: true } }),
        gridPrefs: () => ({ cols: 5 }),
        gridLayout: () => "masonry",
        exportPrefs: () => ({ edge: 2048, border: true, folder: "/exports" }),
      };
      const initCullingState = vi.fn();
      const exportState = { edge: 0, border: false, folder: "" };

      initSessionBoot({ session, layouts, initCullingState, exportState });

      expect(layouts.cull.sidebar).toBe(false);
      expect(initCullingState).toHaveBeenCalledWith({
        gridPrefs: { cols: 5 },
        gridLayout: "masonry",
      });
      expect(exportState.edge).toBe(2048);
      expect(exportState.border).toBe(true);
      expect(exportState.folder).toBe("/exports");
    });
  });

  describe("performAppBoot", () => {
    it("runs Tauri initialization sequence and opens initial dir", async () => {
      const invoke = vi.fn().mockImplementation((cmd) => {
        if (cmd === "default_recipe") return Promise.resolve({ contrast: 0 });
        if (cmd === "list_profiles") return Promise.resolve([
          { stage: "filming", name: "film1" },
          { stage: "printing", name: "paper1" },
        ]);
        if (cmd === "list_luts") return Promise.resolve(["lut1"]);
        if (cmd === "list_engines") return Promise.resolve(["engine1"]);
        if (cmd === "gpu_available") return Promise.resolve(true);
        if (cmd === "load_shell_prefs") return Promise.resolve({ focus_mode: true, auto_import: true, garden_username: "bob" });
        if (cmd === "garden_refresh") return Promise.resolve({ signed_in: true, username: "bob" });
        if (cmd === "autoload_path") return Promise.resolve(null);
        if (cmd === "take_open_file") return Promise.resolve("/test/photo.raw");
        return Promise.resolve(null);
      });

      const developState = {};
      const loadExternalEditors = vi.fn().mockResolvedValue([]);
      const refreshDirs = vi.fn().mockResolvedValue(undefined);
      const session = { lastDirectory: () => "/saved/dir" };
      const library = { dirs: [{ dir: "/saved/dir" }] };
      const openDir = vi.fn().mockResolvedValue(undefined);
      const rescan = vi.fn();
      const layouts = { cull: { focus: false } };
      const currentMode = "cull";
      const importState = {};
      const loadPreferences = vi.fn().mockResolvedValue({ export_folder: "/out" });
      const settingsState = { set: vi.fn() };
      const exportState = {};
      const initCullingState = vi.fn();
      const setGpuAvailable = vi.fn();
      const setGardenAccount = vi.fn();
      const openPhoto = vi.fn().mockResolvedValue(undefined);
      const openFolder = vi.fn().mockResolvedValue(undefined);

      await performAppBoot({
        invoke,
        developState,
        loadExternalEditors,
        refreshDirs,
        session,
        library,
        openDir,
        rescan,
        layouts,
        currentMode,
        importState,
        loadPreferences,
        settingsState,
        exportState,
        initCullingState,
        setGpuAvailable,
        setGardenAccount,
        openPhoto,
        openFolder,
      });

      expect(developState.recipe).toEqual({ contrast: 0 });
      expect(developState.films).toEqual([{ stage: "filming", name: "film1" }]);
      expect(developState.papers).toEqual([{ stage: "printing", name: "paper1" }]);
      expect(setGpuAvailable).toHaveBeenCalledWith(true);
      expect(openDir).toHaveBeenCalledWith("/saved/dir", false, true);
      expect(layouts.cull.focus).toBe(true);
      expect(importState.autoImport).toBe(true);
      expect(settingsState.set).toHaveBeenCalledWith({ export_folder: "/out" });
      expect(exportState.folder).toBe("/out");
      expect(openPhoto).toHaveBeenCalledWith("/test/photo.raw");
    });

    it("opens the last folder without waiting for the full (story) refresh of the tree", async () => {
      const invoke = vi.fn().mockResolvedValue(null);
      // The light refresh answers; the full one (the NAS reads) never does.
      const refreshDirs = vi.fn((light) => (light ? Promise.resolve(true) : new Promise(() => {})));
      const openDir = vi.fn().mockResolvedValue(undefined);
      const boot = performAppBoot({
        invoke,
        developState: {},
        loadExternalEditors: vi.fn().mockResolvedValue([]),
        refreshDirs,
        session: { lastDirectory: () => "/saved/dir" },
        library: { dirs: [{ dir: "/saved/dir" }] },
        openDir,
        rescan: vi.fn(),
        layouts: { cull: { focus: false } },
        currentMode: "cull",
        importState: {},
        loadPreferences: vi.fn().mockResolvedValue({}),
        settingsState: { set: vi.fn() },
        exportState: {},
        initCullingState: vi.fn(),
        setGpuAvailable: vi.fn(),
        setGardenAccount: vi.fn(),
        openPhoto: vi.fn(),
        openFolder: vi.fn(),
      });
      await boot;
      expect(openDir).toHaveBeenCalledWith("/saved/dir", false, true);
      expect(refreshDirs).toHaveBeenNthCalledWith(1, true);
      expect(refreshDirs).toHaveBeenCalledTimes(2); // the full one was started, not awaited
    });
  });
});
