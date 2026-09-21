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
  import SettingsPanel from "@modules/settings/SettingsPanel.svelte";

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
   * @property {string} ai_api_key
   * @property {string} [ai_model]
   * @property {number} [apple_photos_cache_limit_gib]
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
    ai_api_key: "",
    ai_model: "",
    apple_photos_cache_limit_gib: 4,
  });
  let cacheAvailable = $state(false);

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

  onMount(() => {
    if (!isTauri) return;
    invoke("apple_photos_status", { authorize: false })
      .then((result) => { cacheAvailable = /** @type {any} */ (result).supported; })
      .catch(() => {});

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
/>
