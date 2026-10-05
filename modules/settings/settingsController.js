/**
 * Settings Controller.
 *
 * Coordinates standalone Settings window lifecycle, preferences synchronization,
 * Garden account credentials, and preference directory selection.
 */

import {
  sendSettingsToPanel as opSendSettingsToPanel,
  openSettingsWindow as opOpenSettingsWindow,
  savePreferences as opSavePreferences,
  gardenSignIn as opGardenSignIn,
  gardenSignOut as opGardenSignOut,
  choosePreferenceFolder as opChoosePreferenceFolder,
} from "./settingsOperations.js";

/**
 * Creates a bound settings controller instance.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   emit: (event: string, payload: any) => Promise<any>,
 *   isTauri: boolean,
 *   WebviewWindow?: any,
 *   getPreferences: () => any,
 *   setPreferences: (prefs: any) => void,
 *   settingsState: any,
 *   exportState: { folder: string },
 *   saveExportPrefs: () => void,
 *   initCullingState: (opts: { preferences: any }) => void,
 *   setGardenAccount: (acc: any) => void,
 * }} deps
 */
export function createSettingsController(deps) {
  const {
    invoke,
    emit,
    isTauri,
    WebviewWindow,
    getPreferences,
    setPreferences,
    settingsState,
    exportState,
    saveExportPrefs,
    initCullingState,
    setGardenAccount,
  } = deps;

  function sendSettingsToPanel() {
    opSendSettingsToPanel({ emit, isTauri, preferences: getPreferences() });
  }

  async function openSettings() {
    await opOpenSettingsWindow({
      invoke,
      emit,
      isTauri,
      WebviewWindow,
      state: {
        get preferences() {
          return getPreferences();
        },
        set preferences(v) {
          setPreferences(v);
          settingsState.set(v);
        },
      },
      exportFolder: exportState.folder,
    });
  }

  /** @param {any} newPreferences */
  async function saveSettingsFromPanel(newPreferences) {
    setPreferences(newPreferences);
    settingsState.set(newPreferences);
    await opSavePreferences(newPreferences, { invoke, isTauri });
    exportState.folder = newPreferences.export_folder ?? "";
    saveExportPrefs();
    initCullingState({ preferences: newPreferences });
  }

  /** @param {string} apiKey */
  async function gardenSignIn(apiKey) {
    const acc = await opGardenSignIn(apiKey, { invoke });
    setGardenAccount(acc);
    return acc;
  }

  async function gardenSignOut() {
    const acc = await opGardenSignOut({ invoke });
    setGardenAccount(acc);
    return acc;
  }

  /** @param {string} url */
  async function openUrl(url) {
    if (isTauri) await invoke("open_path", { path: url });
    else window.open(url, "_blank");
  }

  /** @param {string} key */
  async function choosePreferenceFolder(key) {
    await opChoosePreferenceFolder(key, {
      invoke,
      emit,
      isTauri,
      state: {
        update: (partial) => {
          const updated = { ...getPreferences(), ...partial };
          setPreferences(updated);
          settingsState.update(partial);
        },
      },
    });
  }

  return {
    sendSettingsToPanel,
    openSettings,
    saveSettingsFromPanel,
    gardenSignIn,
    gardenSignOut,
    openUrl,
    choosePreferenceFolder,
  };
}
