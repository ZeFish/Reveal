/**
 * Settings operations and window lifecycle management.
 *
 * Implements loading, saving, window orchestration, and IPC event emission
 * for Settings.
 */

import { DEFAULT_PREFERENCES } from "./settingsState.svelte.js";

/**
 * Loads preferences from backend and merges with defaults.
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function loadPreferences({ invoke }) {
  try {
    const saved = await invoke("load_preferences");
    return { ...DEFAULT_PREFERENCES, ...(saved || {}) };
  } catch (_) {
    return { ...DEFAULT_PREFERENCES };
  }
}

/**
 * Saves preferences to backend.
 * @param {import("./settingsState.svelte.js").Preferences} preferences
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   isTauri?: boolean,
 * }} options
 */
export async function savePreferences(preferences, { invoke, isTauri = true }) {
  if (isTauri) {
    await invoke("save_preferences", { preferences });
  }
  return preferences;
}

/**
 * Broadcasts current preferences to the detached Settings window.
 * @param {{
 *   emit: (event: string, payload: any) => Promise<any>,
 *   isTauri?: boolean,
 *   preferences: import("./settingsState.svelte.js").Preferences,
 * }} options
 */
export function sendSettingsToPanel({ emit, isTauri = true, preferences }) {
  if (isTauri) {
    emit("main-settings-state", { preferences }).catch(() => {});
  }
}

/**
 * Opens or focuses the detached Settings window.
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   emit: (event: string, payload: any) => Promise<any>,
 *   isTauri?: boolean,
 *   WebviewWindow: any,
 *   state?: { preferences: any },
 *   exportFolder?: string,
 * }} options
 */
export async function openSettingsWindow({
  invoke,
  emit,
  isTauri = true,
  WebviewWindow,
  state,
  exportFolder,
}) {
  if (isTauri && state) {
    const saved = await loadPreferences({ invoke });
    state.preferences = {
      ...state.preferences,
      ...saved,
      export_folder: saved.export_folder ?? exportFolder ?? state.preferences.export_folder,
    };
  }
  if (!isTauri || !WebviewWindow) return;

  const currentPrefs = state?.preferences ?? DEFAULT_PREFERENCES;
  let win = await WebviewWindow.getByLabel("settings-panel");
  if (win) {
    await win.show();
    await win.setFocus();
    sendSettingsToPanel({ emit, isTauri, preferences: currentPrefs });
  } else {
    win = new WebviewWindow("settings-panel", {
      url: "/settings-panel",
      title: "Settings",
      width: 860,
      height: 580,
      minWidth: 720,
      minHeight: 460,
      resizable: true,
      titleBarStyle: "overlay",
      hiddenTitle: true,
    });
    win.once("tauri://created", () => {
      setTimeout(() => {
        sendSettingsToPanel({ emit, isTauri, preferences: currentPrefs });
      }, 300);
    });
  }
  return win;
}

/**
 * Picks a folder for a preference key and emits update to settings panel.
 * @param {string} key
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   emit: (event: string, payload: any) => Promise<any>,
 *   isTauri?: boolean,
 *   state?: { update: (partial: any) => void },
 * }} options
 */
export async function choosePreferenceFolder(key, { invoke, emit, isTauri = true, state }) {
  if (!isTauri) return null;
  const path = await invoke("pick_folder");
  if (path) {
    if (state?.update) {
      state.update({ [key]: path });
    }
    emit("settings-panel-folder-chosen", { key, path }).catch(() => {});
  }
  return path;
}

/**
 * Checks Apple Photos cache status.
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function cacheStatus({ invoke }) {
  return await invoke("apple_photos_cache_status");
}

/**
 * Clears Apple Photos cache.
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function cacheClear({ invoke }) {
  return await invoke("apple_photos_cache_clear");
}

/**
 * Checks developed preview cache status.
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function previewCacheStatus({ invoke }) {
  return await invoke("developed_preview_cache_status");
}

/**
 * Clears developed preview cache.
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function previewCacheClear({ invoke }) {
  return await invoke("developed_preview_cache_clear");
}

/**
 * Signs in to Garden with API key.
 * @param {string} apiKey
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function gardenSignIn(apiKey, { invoke }) {
  return await invoke("garden_sign_in", { apiKey });
}

/**
 * Signs out from Garden.
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function gardenSignOut({ invoke }) {
  return await invoke("garden_sign_out");
}
