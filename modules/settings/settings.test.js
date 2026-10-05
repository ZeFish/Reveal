import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  settingsState,
  DEFAULT_PREFERENCES,
} from "./settingsState.svelte.js";
import {
  loadPreferences,
  savePreferences,
  sendSettingsToPanel,
  openSettingsWindow,
  choosePreferenceFolder,
  cacheStatus,
  cacheClear,
  previewCacheStatus,
  previewCacheClear,
  gardenSignIn,
  gardenSignOut,
} from "./settingsOperations.js";

describe("settingsState", () => {
  beforeEach(() => {
    settingsState.reset();
  });

  it("initializes with default preferences", () => {
    expect(settingsState.preferences.app_theme).toBe("macos");
    expect(settingsState.preferences.date_folders).toBe("%Y/%Y-%m-%d");
    expect(settingsState.preferences.ai_cull_target).toBe(24);
  });

  it("updates individual preferences reactively", () => {
    settingsState.update({ app_theme: "kernel", ai_cull_target: 30 });
    expect(settingsState.preferences.app_theme).toBe("kernel");
    expect(settingsState.preferences.ai_cull_target).toBe(30);
    expect(settingsState.preferences.date_folders).toBe("%Y/%Y-%m-%d");
  });

  it("resets preferences to defaults", () => {
    settingsState.update({ app_theme: "kernel" });
    settingsState.reset();
    expect(settingsState.preferences.app_theme).toBe("macos");
  });
});

describe("settingsOperations", () => {
  it("loadPreferences loads and merges with defaults", async () => {
    const invoke = vi.fn().mockResolvedValue({ app_theme: "academic", vault: "/nas/vault" });
    const prefs = await loadPreferences({ invoke });
    expect(prefs.app_theme).toBe("academic");
    expect(prefs.vault).toBe("/nas/vault");
    expect(prefs.date_folders).toBe("%Y/%Y-%m-%d");
  });

  it("loadPreferences falls back to defaults on error", async () => {
    const invoke = vi.fn().mockRejectedValue(new Error("fail"));
    const prefs = await loadPreferences({ invoke });
    expect(prefs).toEqual(DEFAULT_PREFERENCES);
  });

  it("savePreferences invokes backend command", async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    const prefs = { ...DEFAULT_PREFERENCES, vault: "/my/vault" };
    await savePreferences(prefs, { invoke, isTauri: true });
    expect(invoke).toHaveBeenCalledWith("save_preferences", { preferences: prefs });
  });

  it("sendSettingsToPanel emits IPC event", () => {
    const emit = vi.fn().mockResolvedValue(true);
    const prefs = { ...DEFAULT_PREFERENCES };
    sendSettingsToPanel({ emit, isTauri: true, preferences: prefs });
    expect(emit).toHaveBeenCalledWith("main-settings-state", { preferences: prefs });
  });

  it("openSettingsWindow focuses existing window if found", async () => {
    const mockWin = {
      show: vi.fn().mockResolvedValue(true),
      setFocus: vi.fn().mockResolvedValue(true),
    };
    const WebviewWindow = {
      getByLabel: vi.fn().mockResolvedValue(mockWin),
    };
    const emit = vi.fn().mockResolvedValue(true);
    const invoke = vi.fn().mockResolvedValue({});

    await openSettingsWindow({
      invoke,
      emit,
      isTauri: true,
      WebviewWindow,
      state: settingsState,
    });

    expect(WebviewWindow.getByLabel).toHaveBeenCalledWith("settings-panel");
    expect(mockWin.show).toHaveBeenCalled();
    expect(mockWin.setFocus).toHaveBeenCalled();
    expect(emit).toHaveBeenCalledWith("main-settings-state", expect.any(Object));
  });

  it("choosePreferenceFolder picks folder and updates state", async () => {
    const invoke = vi.fn().mockResolvedValue("/nas/export");
    const emit = vi.fn().mockResolvedValue(true);
    const state = { update: vi.fn() };

    const chosen = await choosePreferenceFolder("export_folder", {
      invoke,
      emit,
      isTauri: true,
      state,
    });

    expect(invoke).toHaveBeenCalledWith("pick_folder");
    expect(state.update).toHaveBeenCalledWith({ export_folder: "/nas/export" });
    expect(emit).toHaveBeenCalledWith("settings-panel-folder-chosen", {
      key: "export_folder",
      path: "/nas/export",
    });
    expect(chosen).toBe("/nas/export");
  });

  it("cache and garden helpers invoke backend", async () => {
    const invoke = vi.fn().mockImplementation(async (cmd, args) => {
      if (cmd === "apple_photos_cache_status") return { size: 100 };
      if (cmd === "garden_sign_in") return { signed_in: true, username: "frank" };
      return true;
    });

    const status = await cacheStatus({ invoke });
    expect(status).toEqual({ size: 100 });

    const user = await gardenSignIn("key-123", { invoke });
    expect(user).toEqual({ signed_in: true, username: "frank" });

    await cacheClear({ invoke });
    expect(invoke).toHaveBeenCalledWith("apple_photos_cache_clear");

    await previewCacheStatus({ invoke });
    expect(invoke).toHaveBeenCalledWith("developed_preview_cache_status");

    await previewCacheClear({ invoke });
    expect(invoke).toHaveBeenCalledWith("developed_preview_cache_clear");

    await gardenSignOut({ invoke });
    expect(invoke).toHaveBeenCalledWith("garden_sign_out");
  });
});
