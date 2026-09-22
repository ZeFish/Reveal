<script>
  // Settings' DETACHED host — always detached, no docked mode (Francis: this
  // should feel like any other app's Settings, always its own window, never
  // a dialog over the main one). Bridges SettingsPanel.svelte's props to the
  // main window over Tauri events, since a separate OS window shares no
  // memory with it. Unlike the Develop panel, this isn't a hot continuous
  // stream — one snapshot in, one save (or a folder-pick round trip) out.
  import { onMount } from "svelte";
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { isTauri } from "$lib/api.js";
  import { applyTheme } from "$lib/app-theme.js";
  import gardenThemes from "$lib/garden-themes.generated.json";
  import SettingsPanel from "@modules/settings/SettingsPanel.svelte";

  const themes = gardenThemes.themes.map((t) => ({ id: String(t.id), label: t.label }));

  /**
   * @typedef {Object} Preferences
   * @property {string} date_folders
   * @property {boolean} [obsidian_enabled]
   * @property {string} vault
   * @property {string} logs_folder
   * @property {string} export_folder
   * @property {string} lut_folder
   * @property {boolean} ai_cull_mark_story
   * @property {boolean} ai_cull_export_desktop
   * @property {number} ai_cull_target
   * @property {string} [ai_provider]
   * @property {string} ai_api_key
   * @property {string} [ai_model]
   * @property {number} [apple_photos_cache_limit_gib]
   * @property {string} [default_engine]
   * @property {string} [app_theme]
   */

  /** @type {Preferences} */
  let preferences = $state({
    date_folders: "%Y/%Y-%m-%d",
    obsidian_enabled: false,
    vault: "",
    logs_folder: "Logs",
    export_folder: "",
    lut_folder: "",
    ai_cull_mark_story: false,
    ai_cull_export_desktop: false,
    ai_cull_target: 24,
    ai_provider: "anthropic",
    ai_api_key: "",
    ai_model: "",
    apple_photos_cache_limit_gib: 4,
    default_engine: "",
    app_theme: "reveal",
  });
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

  const cacheStatus = () => invoke("apple_photos_cache_status");
  const cacheClear = () => invoke("apple_photos_cache_clear");
  const previewCacheStatus = () => invoke("developed_preview_cache_status");
  const previewCacheClear = () => invoke("developed_preview_cache_clear");

  // Libraries. These commands reach the same catalogue from any window, so
  // they run here directly; only the MAIN window's folder tree needs telling,
  // since it is the one showing a library that just appeared or left.
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
    applyTheme(id); // instant preview in this window
    try {
      await invoke("save_preferences", { preferences: { app_theme: id } });
      emit("app-theme-changed", { app_theme: id }); // sync every other open window
    } catch (_) {
      // preferences.app_theme still reflects the pick locally even if the
      // write failed — next save/reload reconciles from disk.
    }
  }

  /** @param {string | null} name */
  async function setDefaultImportPreset(name) {
    defaultImportPreset = name; // optimistic; shell-prefs-changed confirms
    try {
      await invoke("set_default_import_preset", { name });
    } catch (_) {
      // reverted by the next shell-prefs-changed broadcast if this failed
    }
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

    // A folder was chosen in response to our own request above — merge just
    // that one field rather than waiting on a full state resend.
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
</script>

<SettingsPanel
  bind:preferences
  onClose={close}
  onChooseFolder={chooseFolder}
  onSave={save}
  {cacheAvailable}
  onCacheStatus={cacheStatus}
  onCacheClear={cacheClear}
  {autoImportEnabled}
  onToggleAutoImport={toggleAutoImport}
  onPreviewCacheStatus={previewCacheStatus}
  onPreviewCacheClear={previewCacheClear}
  onListLibraries={listLibraries}
  onAddLibrary={addLibrary}
  onRemoveLibrary={removeLibrary}
  onRescanLibrary={rescanLibrary}
  {gardenAccount}
  onGardenSignOut={gardenSignOut}
  {engines}
  {presets}
  {defaultImportPreset}
  onSetDefaultImportPreset={setDefaultImportPreset}
  {themes}
  onSelectTheme={selectTheme}
/>
