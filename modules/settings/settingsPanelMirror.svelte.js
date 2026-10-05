import { onMount } from "svelte";
import { listen, emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri, applyTheme, applyTextSize, GARDEN_THEMES } from "@modules/core";
import { DEFAULT_PREFERENCES } from "./settingsState.svelte.js";
import {
  cacheStatus as opCacheStatus,
  cacheClear as opCacheClear,
  previewCacheStatus as opPreviewCacheStatus,
  previewCacheClear as opPreviewCacheClear,
} from "./settingsOperations.js";

/**
 * Controller for the detached Settings window host.
 *
 * Synchronizes local mirror state with backend Tauri commands and the main window.
 */
export function createSettingsPanelMirror() {
  const themes = GARDEN_THEMES.map((t) => ({ id: String(t.id), label: t.label }));

  /** @type {import('./settingsState.svelte.js').Preferences} */
  let preferences = $state({ ...DEFAULT_PREFERENCES });
  let cacheAvailable = $state(false);
  let autoImportEnabled = $state(false);
  /** @type {{signed_in: boolean, username?: string, tier?: string, notes_count?: number, total_views?: number} | null} */
  let gardenAccount = $state(null);
  /** @type {{id: string, label: string}[]} */
  let engines = $state([]);
  /** @type {{name: string, recipe: any}[]} */
  let presets = $state([]);
  /** @type {string | null} */
  let defaultImportPreset = $state(null);

  function close() {
    if (isTauri) getCurrentWindow().close();
  }

  /** @param {string} key */
  function chooseFolder(key) {
    emit("settings-panel-choose-folder", { key });
  }

  function save() {
    emit("settings-panel-save", { preferences: { ...preferences } });
    close();
  }

  const cacheStatus = () => opCacheStatus({ invoke });
  const cacheClear = () => opCacheClear({ invoke });
  const previewCacheStatus = () => opPreviewCacheStatus({ invoke });
  const previewCacheClear = () => opPreviewCacheClear({ invoke });

  const listLibraries = () => invoke("catalog_roots");
  /** @param {string} path */
  const removeLibrary = async (path) => {
    await invoke("remove_catalog_root", { path });
    emit("libraries-changed", {});
  };
  /** @param {string} path */
  const rescanLibrary = async (path) => {
    await invoke("scan_root", { path });
    emit("libraries-changed", {});
  };
  const addLibrary = async () => {
    const path = await invoke("pick_folder");
    if (!path) return;
    await invoke("add_catalog_root", { path });
    emit("libraries-changed", {});
  };

  /** @param {boolean} enabled */
  async function toggleAutoImport(enabled) {
    const prefs = /** @type {any} */ (await invoke("set_auto_import", { enabled }));
    autoImportEnabled = !!prefs.auto_import;
  }

  async function gardenSignOut() {
    gardenAccount = /** @type {any} */ (await invoke("garden_sign_out"));
  }

  /** @param {string} id */
  async function selectTheme(id) {
    preferences = { ...preferences, app_theme: id };
    applyTheme(id);
    try {
      await invoke("save_preferences", { preferences: { app_theme: id } });
      emit("app-theme-changed", { app_theme: id });
    } catch (_) {}
  }

  /** @param {number} px */
  async function selectTextSize(px) {
    preferences = { ...preferences, ui_text_size: px };
    applyTextSize(px);
    try {
      await invoke("save_preferences", { preferences: { ui_text_size: px } });
      emit("app-text-size-changed", { ui_text_size: px });
    } catch (_) {}
  }

  /** @param {string | null} name */
  async function setDefaultImportPreset(name) {
    defaultImportPreset = name;
    try {
      await invoke("set_default_import_preset", { name });
    } catch (_) {}
  }

  onMount(() => {
    if (!isTauri) return;
    invoke("apple_photos_status", { authorize: false })
      .then((result) => { cacheAvailable = /** @type {any} */ (result).supported; })
      .catch(() => {});
    invoke("load_shell_prefs")
      .then((result) => {
        autoImportEnabled = !!(/** @type {any} */ (result).auto_import);
        defaultImportPreset = /** @type {any} */ (result).default_import_preset ?? null;
      })
      .catch(() => {});
    invoke("garden_refresh").then((info) => (gardenAccount = /** @type {any} */ (info))).catch(() => {});
    invoke("list_engines").then((list) => (engines = /** @type {any} */ (list))).catch(() => {});
    invoke("hide_window_traffic_lights").catch(() => {});
    (async () => {
      try {
        const win = getCurrentWindow();
        const factor = await win.scaleFactor();
        const size = await win.innerSize();
        const w = size.width / factor;
        const h = size.height / factor;
        if (w < 820 || h < 540) {
          const { LogicalSize } = await import("@tauri-apps/api/dpi");
          await win.setSize(new LogicalSize(860, 580));
        }
      } catch (_) {}
    })();

    invoke("list_presets").then((list) => (presets = /** @type {any} */ (list))).catch(() => {});
    const unlistenGarden = listen("garden-account-changed", (e) => (gardenAccount = /** @type {any} */ (e.payload)));
    const unlistenPresetsChanged = listen("presets-changed", () => {
      invoke("list_presets").then((list) => (presets = /** @type {any} */ (list))).catch(() => {});
    });
    const unlistenShellPrefs = listen("shell-prefs-changed", (e) => {
      autoImportEnabled = !!(/** @type {any} */ (e.payload).auto_import);
      defaultImportPreset = /** @type {any} */ (e.payload).default_import_preset ?? null;
    });

    const unlisten = listen("main-settings-state", (e) => {
      preferences = { ...preferences, ...(/** @type {any} */ (e.payload).preferences ?? {}) };
    });
    unlisten.then(() => emit("settings-panel-ready", {}));

    const unlistenFolder = listen("settings-panel-folder-chosen", (e) => {
      const payload = /** @type {any} */ (e.payload);
      if (payload?.key && payload.path) {
        preferences = { ...preferences, [payload.key]: payload.path };
      }
    });

    return () => {
      unlisten.then((fn) => fn());
      unlistenFolder.then((fn) => fn());
      unlistenGarden.then((fn) => fn());
      unlistenPresetsChanged.then((fn) => fn());
      unlistenShellPrefs.then((fn) => fn());
    };
  });

  return {
    get preferences() { return preferences; },
    set preferences(v) { preferences = v; },
    get cacheAvailable() { return cacheAvailable; },
    get autoImportEnabled() { return autoImportEnabled; },
    get gardenAccount() { return gardenAccount; },
    get engines() { return engines; },
    get presets() { return presets; },
    get defaultImportPreset() { return defaultImportPreset; },
    get themes() { return themes; },

    close,
    chooseFolder,
    save,
    cacheStatus,
    cacheClear,
    previewCacheStatus,
    previewCacheClear,
    listLibraries,
    addLibrary,
    removeLibrary,
    rescanLibrary,
    toggleAutoImport,
    gardenSignOut,
    selectTheme,
    selectTextSize,
    setDefaultImportPreset,

    get panelProps() {
      return {
        get preferences() { return preferences; },
        set preferences(v) { preferences = v; },
        onClose: close,
        onChooseFolder: chooseFolder,
        onSave: save,
        cacheAvailable,
        onCacheStatus: cacheStatus,
        onCacheClear: cacheClear,
        autoImportEnabled,
        onToggleAutoImport: toggleAutoImport,
        onPreviewCacheStatus: previewCacheStatus,
        onPreviewCacheClear: previewCacheClear,
        onListLibraries: listLibraries,
        onAddLibrary: addLibrary,
        onRemoveLibrary: removeLibrary,
        onRescanLibrary: rescanLibrary,
        gardenAccount,
        onGardenSignOut: gardenSignOut,
        engines,
        presets,
        defaultImportPreset,
        onSetDefaultImportPreset: setDefaultImportPreset,
        themes,
        onSelectTheme: selectTheme,
        onSelectTextSize: selectTextSize,
      };
    },
  };
}
