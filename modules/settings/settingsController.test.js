import { describe, it, expect, vi } from "vitest";
import { createSettingsController } from "./settingsController.js";

describe("createSettingsController", () => {
  it("coordinates preference saving and garden sign-in/out", async () => {
    let prefs = { export_folder: "/exports" };
    let gardenAccount = null;
    const invoke = vi.fn().mockImplementation(async (cmd, args) => {
      if (cmd === "garden_sign_in") return { signed_in: true, username: "francis" };
      if (cmd === "garden_sign_out") return null;
      if (cmd === "save_preferences") return true;
      return null;
    });
    const emit = vi.fn();
    const saveExportPrefs = vi.fn();
    const initCullingState = vi.fn();
    const settingsState = { set: vi.fn(), update: vi.fn() };
    const exportState = { folder: "/exports" };

    const controller = createSettingsController({
      invoke,
      emit,
      isTauri: true,
      getPreferences: () => prefs,
      setPreferences: (p) => { prefs = p; },
      settingsState,
      exportState,
      saveExportPrefs,
      initCullingState,
      setGardenAccount: (acc) => { gardenAccount = acc; },
    });

    // Test saveSettingsFromPanel
    await controller.saveSettingsFromPanel({ export_folder: "/new_exports", theme: "dark" });
    expect(prefs.export_folder).toBe("/new_exports");
    expect(exportState.folder).toBe("/new_exports");
    expect(saveExportPrefs).toHaveBeenCalled();
    expect(initCullingState).toHaveBeenCalled();
    expect(settingsState.set).toHaveBeenCalled();

    // Test gardenSignIn
    const account = await controller.gardenSignIn("secret_key");
    expect(account.signed_in).toBe(true);
    expect(gardenAccount.username).toBe("francis");

    // Test gardenSignOut
    await controller.gardenSignOut();
    expect(gardenAccount).toBeNull();
  });
});
