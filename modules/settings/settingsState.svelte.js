/**
 * Settings state store using Svelte 5 runes.
 *
 * Encapsulates application preferences, external API configuration,
 * and Settings window synchronization.
 */

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
 * @property {number} [ui_text_size]
 * @property {string} [immich_url]
 * @property {string} [immich_api_key]
 * @property {boolean} [immich_export_enabled]
 * @property {string} [google_photos_client_id]
 * @property {string} [google_photos_client_secret]
 * @property {string} [google_photos_refresh_token]
 * @property {boolean} [google_photos_export_enabled]
 */

/** @type {Preferences} */
export const DEFAULT_PREFERENCES = {
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
  app_theme: "macos",
  immich_url: "",
  immich_api_key: "",
  immich_export_enabled: false,
  google_photos_client_id: "",
  google_photos_client_secret: "",
  google_photos_refresh_token: "",
  google_photos_export_enabled: false,
};

class SettingsState {
  /** @type {Preferences} */
  preferences = $state({ ...DEFAULT_PREFERENCES });

  cacheAvailable = $state(false);
  autoImportEnabled = $state(false);
  /** @type {{signed_in: boolean, username?: string, tier?: string, notes_count?: number, total_views?: number} | null} */
  gardenAccount = $state(null);
  /** @type {{id: string, label: string}[]} */
  engines = $state([]);
  /** @type {{name: string, recipe: any}[]} */
  presets = $state([]);
  /** @type {string | null} */
  defaultImportPreset = $state(null);

  /**
   * Updates one or more preference keys.
   * @param {Partial<Preferences>} partial
   */
  update(partial) {
    this.preferences = { ...this.preferences, ...partial };
  }

  /**
   * Replaces all preferences.
   * @param {Preferences} newPrefs
   */
  set(newPrefs) {
    this.preferences = { ...DEFAULT_PREFERENCES, ...newPrefs };
  }

  /**
   * Resets preferences back to defaults.
   */
  reset() {
    this.preferences = { ...DEFAULT_PREFERENCES };
  }
}

export const settingsState = new SettingsState();
