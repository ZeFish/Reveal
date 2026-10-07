<script>
  // The Settings interface — a real window, macOS System Settings layout:
  // categories down the left, that category's view on the right.
  // Each domain's settings are co-located in its own vertical slice
  // (Immich in @modules/immich, Library in @modules/library, etc.).
  import Alert from "@stnd/ui/Alert.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Icon, isTauri } from "@modules/core";

  // Category view components from vertical slices
  import GeneralSettings from "./categories/GeneralSettings.svelte";
  import { LibrarySettings } from "@modules/library";
  import { ImportExportSettings } from "@modules/import";
  import GardenSettings from "./categories/GardenSettings.svelte";
  import { ObsidianSettings } from "@modules/export";
  import { ImmichSettings } from "@modules/immich";
  import GooglePhotosSettings from "./categories/GooglePhotosSettings.svelte";
  import StorageSettings from "./categories/StorageSettings.svelte";
  import { AiSettings } from "@modules/culling";

  /** @typedef {import('./settingsState.svelte.js').Preferences} Preferences */

  /** @typedef {Object} EngineInfo
   * @property {string} id
   * @property {string} label
   */

  /** @typedef {Object} GardenAccount
   * @property {boolean} signed_in
   * @property {string} [username]
   * @property {string} [tier]
   * @property {number} [notes_count]
   * @property {number} [total_views]
   */

  /**
   * @typedef {Object} Props
   * @property {Preferences} [preferences]
   * @property {() => void} [onClose]
   * @property {(key: string) => void} [onChooseFolder]
   * @property {() => Promise<void> | void} [onSave]
   * @property {boolean} [cacheAvailable]
   * @property {() => Promise<{size_bytes: number, limit_bytes: number, in_use_bytes: number}>} [onCacheStatus]
   * @property {() => Promise<{removed_bytes: number, remaining_bytes: number, protected_bytes: number}>} [onCacheClear]
   * @property {boolean} [autoImportEnabled]
   * @property {(enabled: boolean) => Promise<void> | void} [onToggleAutoImport]
   * @property {{name: string, recipe: any}[]} [presets]
   * @property {string | null} [defaultImportPreset]
   * @property {(name: string | null) => Promise<void> | void} [onSetDefaultImportPreset]
   * @property {{id: string, label: string}[]} [themes]
   * @property {(id: string) => Promise<void> | void} [onSelectTheme]
   * @property {(px: number) => void} [onSelectTextSize]
   * @property {() => Promise<{size_bytes: number, photo_count: number, limit_bytes: number}>} [onPreviewCacheStatus]
   * @property {() => Promise<{removed_bytes: number}>} [onPreviewCacheClear]
   * @property {() => Promise<{path: string, frames: number, online: boolean}[]>} [onListLibraries]
   * @property {() => Promise<void> | void} [onAddLibrary]
   * @property {(path: string) => Promise<void> | void} [onRemoveLibrary]
   * @property {(path: string) => Promise<void> | void} [onRescanLibrary]
   * @property {GardenAccount | null} [gardenAccount]
   * @property {() => Promise<void>} [onGardenSignOut]
   * @property {EngineInfo[]} [engines]
   */

  /** @type {Props} */
  let {
    preferences = $bindable(
      /** @type {Preferences} */ ({
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
        immich_url: "",
        immich_api_key: "",
        immich_export_enabled: false,
        google_photos_client_id: "",
        google_photos_client_secret: "",
        google_photos_refresh_token: "",
        google_photos_export_enabled: false,
      })
    ),
    onClose = () => {},
    /** @type {(key: string) => void} */
    onChooseFolder = () => {},
    onSave = () => {},
    cacheAvailable = false,
    /** @type {() => Promise<{size_bytes: number, limit_bytes: number, in_use_bytes: number}>} */
    onCacheStatus = async () => ({ size_bytes: 0, limit_bytes: 0, in_use_bytes: 0 }),
    /** @type {() => Promise<{removed_bytes: number, remaining_bytes: number, protected_bytes: number}>} */
    onCacheClear = async () => ({ removed_bytes: 0, remaining_bytes: 0, protected_bytes: 0 }),
    autoImportEnabled = false,
    /** @type {(enabled: boolean) => Promise<void> | void} */
    onToggleAutoImport = () => {},
    /** @type {{name: string, recipe: any}[]} */
    presets = [],
    /** @type {string | null} */
    defaultImportPreset = null,
    /** @type {(name: string | null) => Promise<void> | void} */
    onSetDefaultImportPreset = () => {},
    /** @type {{id: string, label: string}[]} */
    themes = [],
    /** @type {(id: string) => Promise<void> | void} */
    onSelectTheme = () => {},
    /** @type {(px: number) => void} */
    onSelectTextSize = () => {},
    /** @type {() => Promise<{size_bytes: number, photo_count: number, limit_bytes: number}>} */
    onPreviewCacheStatus = async () => ({ size_bytes: 0, photo_count: 0, limit_bytes: 0 }),
    /** @type {() => Promise<{removed_bytes: number}>} */
    onPreviewCacheClear = async () => ({ removed_bytes: 0 }),
    /** @type {() => Promise<{path: string, frames: number, online: boolean}[]>} */
    onListLibraries = async () => [],
    /** @type {() => Promise<void> | void} */
    onAddLibrary = () => {},
    /** @type {(path: string) => Promise<void> | void} */
    onRemoveLibrary = () => {},
    /** @type {(path: string) => Promise<void> | void} */
    onRescanLibrary = () => {},
    /** @type {GardenAccount | null} */
    gardenAccount = null,
    /** @type {() => Promise<void>} */
    onGardenSignOut = async () => {},
    /** @type {EngineInfo[]} */
    engines = [],
  } = $props();

  const SECTIONS = [
    {
      title: "",
      items: [
        { id: "general", label: "General", icon: "gear" },
      ],
    },
    {
      title: "PHOTO LIBRARY",
      items: [
        { id: "library", label: "Libraries", icon: "books" },
        { id: "import_export", label: "Import & Export", icon: "arrows-down-up" },
      ],
    },
    {
      title: "INTEGRATIONS",
      items: [
        { id: "garden", label: "Garden Account", icon: "stnd-garden" },
        { id: "obsidian", label: "Obsidian", icon: "note-pencil" },
        { id: "immich", label: "Immich", icon: "cloud-arrow-up" },
        { id: "google_photos", label: "Google Photos", icon: "google-photos-logo" },
      ],
    },
    {
      title: "SYSTEM",
      items: [
        { id: "cache", label: "Cache & Storage", icon: "hard-drive" },
        { id: "ai", label: "AI & Automation", icon: "lightning" },
      ],
    },
  ];

  const visibleCategories = $derived(SECTIONS.flatMap((s) => s.items));
  let activeCategory = $state("general");

  $effect(() => {
    if (!visibleCategories.some((c) => c.id === activeCategory)) {
      activeCategory = visibleCategories[0]?.id ?? "general";
    }
  });

  let saving = $state(false);
  let saveError = $state("");

  async function save() {
    if (saving) return;
    saveError = "";
    const limit = preferences.apple_photos_cache_limit_gib;
    if (cacheAvailable && (limit === undefined || !Number.isInteger(limit) || limit < 1 || limit > 64)) {
      saveError = "Apple Photos cache limit must be a whole number from 1 to 64 GiB.";
      activeCategory = "cache";
      return;
    }
    saving = true;
    try {
      await onSave();
    } catch (error) {
      saveError = `Could not save settings: ${String(error)}`;
    } finally {
      saving = false;
    }
  }

  /** @param {KeyboardEvent} e */
  function handleKeyDown(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      save();
    }
  }

  async function minimizeWindow() {
    if (isTauri) {
      try {
        await getCurrentWindow().minimize();
      } catch (e) {
        console.error("minimizeWindow:", e);
      }
    }
  }

  async function zoomWindow() {
    if (isTauri) {
      try {
        await getCurrentWindow().toggleMaximize();
      } catch (e) {
        console.error("zoomWindow:", e);
      }
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="settings-window">
  {#if isTauri}
    <div class="window-controls-zone" data-tauri-drag-region>
      <div class="window-controls" aria-label="Window controls">
        <button class="window-close" onclick={() => onClose()} aria-label="Close window"></button>
        <button class="window-minimize" onclick={minimizeWindow} aria-label="Minimize window"></button>
        <button class="window-zoom" onclick={zoomWindow} aria-label="Zoom window"></button>
      </div>
    </div>
  {/if}

  <nav class="categories" data-tauri-drag-region>
    <div class="categories-spacer" data-tauri-drag-region></div>
    {#each SECTIONS as section, sIdx}
      {#if section.title}
        <div class="nav-section-title" class:nav-section-spaced={sIdx > 0}>{section.title}</div>
      {/if}
      <div class="nav-section-items">
        {#each section.items as cat (cat.id)}
          <button
            class="item category-btn"
            aria-current={activeCategory === cat.id ? "true" : undefined}
            onclick={() => (activeCategory = cat.id)}
          >
            <Icon name={cat.icon} size="var(--icon-lg)" />
            <span>{cat.label}</span>
          </button>
        {/each}
      </div>
    {/each}
  </nav>

  <div class="detail">
    <div class="detail-scroll">
      {#if activeCategory === "general"}
        <GeneralSettings bind:preferences {themes} {onSelectTheme} {onSelectTextSize} />
      {:else if activeCategory === "library"}
        <LibrarySettings {onListLibraries} {onAddLibrary} {onRemoveLibrary} {onRescanLibrary} />
      {:else if activeCategory === "import_export"}
        <ImportExportSettings
          bind:preferences
          {autoImportEnabled}
          {onToggleAutoImport}
          {presets}
          {defaultImportPreset}
          {onSetDefaultImportPreset}
          {onChooseFolder}
        />
      {:else if activeCategory === "garden"}
        <GardenSettings {gardenAccount} {onGardenSignOut} />
      {:else if activeCategory === "obsidian"}
        <ObsidianSettings bind:preferences {onChooseFolder} />
      {:else if activeCategory === "immich"}
        <ImmichSettings bind:preferences />
      {:else if activeCategory === "google_photos"}
        <GooglePhotosSettings bind:preferences />
      {:else if activeCategory === "cache"}
        <StorageSettings
          bind:preferences
          {cacheAvailable}
          {saving}
          {onCacheStatus}
          {onCacheClear}
          {onPreviewCacheStatus}
          {onPreviewCacheClear}
        />
      {:else if activeCategory === "ai"}
        <AiSettings bind:preferences />
      {/if}
    </div>

    {#if saveError}<div class="save-error" role="alert"><Alert class="error">{saveError}</Alert></div>{/if}
    <footer class="settings-footer">
      <div class="footer-actions">
        <button type="button" class="outline small btn-cancel" onclick={() => onClose()}>CANCEL</button>
        <button type="button" class="accent small btn-save" onclick={save} disabled={saving}>SAVE</button>
      </div>
    </footer>
  </div>
</div>

<style>
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: var(--color-background);
    color: var(--color-foreground);
    overflow: hidden;
  }

  .settings-window {
    height: 100vh;
    display: flex;
    background: var(--color-background);
  }

  /* Categories — sidebar pane for settings */
  .categories {
    width: 200px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-d8);
    margin: 0;
    padding: 0 var(--space-d2) var(--space-d2);
    background: var(--color-surface-light-1);
    border-right: var(--border);
    overflow-y: auto;
  }
  .categories-spacer {
    height: var(--titlebar-height, 42px);
    flex-shrink: 0;
  }
  .nav-section-title {
    font-size: var(--scale-d3, 0.7rem);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-muted);
    opacity: 0.7;
    padding: var(--space-d4) calc(var(--space-d4) * 3) var(--space-d8);
    user-select: none;
  }
  .nav-section-spaced {
    margin-top: var(--space-d3);
    padding-top: var(--space-d3);
    border-top: var(--border);
  }
  .nav-section-items {
    display: flex;
    flex-direction: column;
    gap: var(--space-d8);
  }
  .category-btn {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    width: 100%;
    margin: 0;
    padding: var(--space-d3) calc(var(--space-d4) * 3);
    cursor: pointer;
  }

  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow-inset);
  }
  .detail-scroll {
    padding: var(--titlebar-height, 42px) var(--space) var(--space);
    overflow-y: auto;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 5);
  }
  .save-error {
    padding: 0 calc(var(--space-d4) * 6);
  }

  /* Detail scroll shared styles for all categories */
  .detail-scroll :global(.section-group) {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 2);
  }
  .detail-scroll :global(.section-heading) {
    display: flex;
    align-items: center;
    gap: calc(var(--space-d4) * 2);
    padding-left: var(--space-d4);
  }
  .detail-scroll :global(.date-card) { overflow: visible; }
  .detail-scroll :global(.setting-row) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: calc(var(--space-d4) * 3) var(--space);
    gap: var(--space);
  }
  .detail-scroll :global(.accordion-header) {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: calc(var(--space-d4) * 3) var(--space);
    gap: var(--space);
    cursor: pointer;
    background: transparent;
    transition: background var(--duration-instant) var(--ease-soft);
  }
  .detail-scroll :global(.accordion-header:hover) {
    background: var(--color-surface);
  }
  .detail-scroll :global(.sub-row) {
    background: color-mix(in srgb, var(--color-foreground) 3%, transparent);
    padding-left: calc(var(--space) + var(--space-d4));
  }
  .detail-scroll :global(.disabled-card) {
    opacity: 0.6;
  }
  .detail-scroll :global(.row-meta) {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 2);
    flex: 1;
    min-width: 0;
  }
  .detail-scroll :global(.lib-path) {
    opacity: 0.6;
    overflow-wrap: anywhere;
  }
  .detail-scroll :global(.lib-offline) {
    color: var(--color-accent);
  }
  .detail-scroll :global(.row-control) {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
  }
  /* Every control in a row — pill buttons, selects, text fields — is `--control-h` tall (app.scss),
     the height the Develop panel's bars use. They were 14px pills beside 31px fields. */
  .detail-scroll :global(.mono-input) {
    box-sizing: border-box;
    height: var(--control-h);
    padding: 0 calc(var(--space-d4) * 3);
    width: 190px;
    max-width: 100%;
  }
  .detail-scroll :global(.date-control-col) {
    position: relative;
  }
  .detail-scroll :global(.input-with-presets) {
    display: flex;
    align-items: center;
    position: relative;
  }
  .detail-scroll :global(.input-with-presets .mono-input) {
    padding-right: calc(var(--space-d4) * 7);
    width: 190px;
  }
  .detail-scroll :global(.preset-option) {
    display: flex;
    flex-direction: column;
    gap: var(--space-d8);
    padding: var(--space-d4) 0;
  }
  .detail-scroll :global(.path-picker-group) {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
  }
  .detail-scroll :global(.path-display) {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d3);
    max-width: 240px;
  }
  .detail-scroll :global(.path-text) {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .detail-scroll :global(.clear-path-btn) {
    width: 18px;
    height: 18px;
    padding: 0;
  }
  .detail-scroll :global(.action-pill-btn) {
    white-space: nowrap;
    gap: var(--space-d4);
    min-height: var(--control-h);
    padding-block: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .detail-scroll :global(select) {
    min-height: var(--control-h);
    padding-block: 0;
  }
  .detail-scroll :global(.stepper-group) {
    display: flex;
    align-items: center;
    gap: 1px;
    background: var(--color-surface);
    border: var(--border);
    border-radius: var(--radius-sm);
  }
  .detail-scroll :global(.stepper-btn) {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    cursor: pointer;
    user-select: none;
    background: transparent;
    border: none;
  }
  .detail-scroll :global(.stepper-btn:disabled) {
    opacity: 0.3;
    cursor: default;
  }
  .detail-scroll :global(.stepper-input) {
    width: 44px;
    height: 26px;
    text-align: center;
    border: none;
    background: transparent;
    -moz-appearance: textfield;
    appearance: textfield;
  }
  .detail-scroll :global(.stepper-input::-webkit-outer-spin-button),
  .detail-scroll :global(.stepper-input::-webkit-inner-spin-button) {
    -webkit-appearance: none;
    margin: 0;
  }
  .detail-scroll :global(.cache-actions) { flex-wrap: wrap; gap: var(--space-half); }
  .detail-scroll :global(.version-control) { gap: var(--space-half); }

  /* Footer */
  .settings-footer {
    padding: var(--space) calc(var(--space-d4) * 5);
    border-top: var(--border);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    user-select: none;
  }
  .footer-actions {
    display: flex;
    align-items: center;
    gap: var(--space-d2);
  }
  .footer-actions button {
    min-height: var(--control-h);
    padding-block: 0;
    display: inline-flex;
    align-items: center;
  }
</style>
