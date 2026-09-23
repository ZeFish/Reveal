<script>
  // The Settings interface — a real window, not a dialog over the app (per
  // Francis: settings should feel like any other app's Settings, always its
  // own window). macOS System Settings layout: categories down the left,
  // that category's fields on the right. Adding a category later is adding
  // one entry to CATEGORIES and one {#if} block below — nothing else moves.
  //
  // Same prop contract the old SettingsModal.svelte had (preferences
  // bindable, onClose/onChooseFolder/onSave, onCacheStatus/onCacheClear) —
  // deliberately, so whichever host renders this doesn't need its own
  // translation layer. routes/settings-panel/+page.svelte is that host: it
  // bridges these to the main window over Tauri events, since this window
  // shares no memory with it.
  import Icon from "$lib/components/Icon.svelte";
  import Dropdown from "@stnd/ui/Dropdown.svelte";
  import DropdownItem from "@stnd/ui/DropdownItem.svelte";
  import Alert from "@stnd/ui/Alert.svelte";
  import AlertDialog from "@stnd/ui/AlertDialog.svelte";
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { isTauri } from "$lib/api.js";

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

  const CATEGORIES = [
    { id: "photos", label: "Photos", icon: "image" },
    { id: "appearance", label: "Appearance", icon: "palette" },
    { id: "locations", label: "Locations", icon: "folder-open" },
    { id: "library", label: "Libraries", icon: "books" },
    { id: "obsidian", label: "Obsidian", icon: "note-pencil" },
    { id: "cache", label: "Cache & Storage", icon: "hard-drive" },
    { id: "garden", label: "Garden Account", icon: "user-circle" },
    { id: "ai", label: "AI & Automation", icon: "lightning" },
    { id: "about", label: "About", icon: "info" },
  ];
  const AI_PROVIDERS = [
    { id: "anthropic", label: "Anthropic (Claude)", modelPlaceholder: "claude-sonnet-5" },
    { id: "gemini", label: "Google (Gemini)", modelPlaceholder: "gemini-2.5-flash" },
  ];
  const visibleCategories = $derived(CATEGORIES);
  const aiCullActive = $derived(preferences.ai_cull_mark_story || preferences.ai_cull_export_desktop);
  let activeCategory = $state("photos");
  // A category that stops being visible (e.g. Apple Photos support changing)
  // shouldn't leave the pane on a hidden section.
  $effect(() => {
    if (!visibleCategories.some((c) => c.id === activeCategory)) {
      activeCategory = visibleCategories[0]?.id ?? "photos";
    }
  });

  let appVersion = $state("");
  $effect(() => {
    if (isTauri) getVersion().then((v) => (appVersion = v)).catch(() => {});
  });
  /** @param {string} url */
  function openExternal(url) {
    invoke("open_path", { path: url }).catch(() => {});
  }

  let cacheBusy = $state(false);
  let cacheConfirm = $state(false);
  let saving = $state(false);
  let saveError = $state("");
  let cacheError = $state("");
  let cacheMessage = $state("");
  /** @type {{size_bytes: number, limit_bytes: number, in_use_bytes: number} | null} */
  let cacheStatus = $state(null);
  /** @param {number} bytes */
  const formatBytes = (bytes) => `${(bytes / 1024 ** 3).toLocaleString("en-CA", { maximumFractionDigits: 2 })} GiB`;

  $effect(() => {
    if (cacheAvailable) untrack(() => {
      preferences.apple_photos_cache_limit_gib ??= 4;
      loadCacheStatus();
    });
  });

  let devCacheBusy = $state(false);
  let devCacheConfirm = $state(false);

  // Libraries. `null` means "not read yet", which is different from "none".
  /** @type {{path: string, frames: number, online: boolean}[] | null} */
  let libraries = $state(null);
  let libBusy = $state(false);
  let libError = $state("");
  /** @type {{path: string, frames: number, online: boolean} | null} */
  let libToRemove = $state(null);

  async function loadLibraries() {
    libError = "";
    try {
      libraries = await onListLibraries();
    } catch (e) {
      libError = `Libraries unavailable: ${e}`;
      libraries = [];
    }
  }

  async function addLibrary() {
    libBusy = true;
    try {
      await onAddLibrary();
      await loadLibraries();
    } catch (e) {
      libError = `Could not add that folder: ${e}`;
    } finally {
      libBusy = false;
    }
  }

  /** @param {string} path */
  async function rescanLibrary(path) {
    libBusy = true;
    libError = "";
    try {
      await onRescanLibrary(path);
      await loadLibraries();
    } catch (e) {
      libError = `Reindex failed: ${e}`;
    } finally {
      libBusy = false;
    }
  }

  async function confirmRemoveLibrary() {
    const lib = libToRemove;
    libToRemove = null;
    if (!lib) return;
    libBusy = true;
    libError = "";
    try {
      await onRemoveLibrary(lib.path);
      await loadLibraries();
    } catch (e) {
      libError = `Could not remove that library: ${e}`;
    } finally {
      libBusy = false;
    }
  }

  // Read on first visit rather than at mount: the list costs a COUNT per root
  // over the catalogue, and most trips into Settings never open this tab.
  $effect(() => {
    if (activeCategory === "library" && libraries === null) untrack(loadLibraries);
  });
  let devCacheError = $state("");
  let devCacheMessage = $state("");
  /** @type {{size_bytes: number, photo_count: number, limit_bytes: number} | null} */
  let devCacheStatus = $state(null);

  $effect(() => {
    untrack(() => loadDevCacheStatus());
  });

  async function loadDevCacheStatus() {
    if (devCacheBusy) return;
    devCacheBusy = true;
    devCacheError = "";
    try {
      devCacheStatus = await onPreviewCacheStatus();
    } catch (error) {
      devCacheError = `Could not read preview cache usage: ${String(error)}`;
    } finally {
      devCacheBusy = false;
    }
  }

  async function clearDevCache() {
    devCacheConfirm = false;
    if (devCacheBusy) return;
    devCacheBusy = true;
    devCacheError = "";
    devCacheMessage = "";
    try {
      const result = await onPreviewCacheClear();
      devCacheMessage = `Cleared ${formatBytes(result.removed_bytes)}. RAWs redevelop on next view.`;
      devCacheStatus = await onPreviewCacheStatus();
    } catch (error) {
      devCacheError = devCacheMessage
        ? `Cache was cleared, but usage could not be refreshed: ${String(error)}`
        : `Could not clear preview cache: ${String(error)}`;
    } finally {
      devCacheBusy = false;
    }
  }

  let gardenSigningOut = $state(false);
  let gardenError = $state("");
  async function signOutOfGarden() {
    if (gardenSigningOut) return;
    gardenSigningOut = true;
    gardenError = "";
    try {
      await onGardenSignOut();
    } catch (error) {
      gardenError = `Could not sign out: ${String(error)}`;
    } finally {
      gardenSigningOut = false;
    }
  }

  async function loadCacheStatus() {
    if (cacheBusy) return;
    cacheBusy = true;
    cacheError = "";
    try {
      cacheStatus = await onCacheStatus();
    } catch (error) {
      cacheError = `Could not read Apple Photos cache usage: ${String(error)}`;
    } finally {
      cacheBusy = false;
    }
  }

  async function clearCache() {
    cacheConfirm = false;
    if (cacheBusy) return;
    cacheBusy = true;
    cacheError = "";
    cacheMessage = "";
    try {
      const result = await onCacheClear();
      cacheMessage = `Cleared ${formatBytes(result.removed_bytes)}; ${formatBytes(result.remaining_bytes)} remains.`
        + (result.protected_bytes > 0 ? ` ${formatBytes(result.protected_bytes)} is in use and was kept. Retry after active work finishes.` : "")
        + " Edits, ratings and captions are unchanged.";
      // Usage is derived state, never part of the preferences saved below.
      cacheStatus = await onCacheStatus();
    } catch (error) {
      cacheError = cacheMessage
        ? `Cached copies were cleared, but usage could not be refreshed: ${String(error)}`
        : `Could not complete cache cleanup: ${String(error)}`;
    } finally {
      cacheBusy = false;
    }
  }

  async function save() {
    if (saving || cacheBusy || cacheConfirm) return;
    saveError = "";
    const limit = preferences.apple_photos_cache_limit_gib;
    if (cacheAvailable && (limit === undefined || !Number.isInteger(limit) || limit < 1 || limit > 64)) {
      saveError = "Apple Photos cache limit must be a whole number from 1 to 64 GiB.";
      activeCategory = "apple-photos";
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

  const datePresets = [
    { pattern: "%Y/%Y-%m-%d", example: "2026/2026-12-31", desc: "Year / Year-Month-Day" },
    { pattern: "%Y/%m/%d", example: "2026/12/31", desc: "Year / Month / Day" },
    { pattern: "%Y-%m-%d", example: "2026-12-31", desc: "Year-Month-Day (flat)" },
    { pattern: "%Y/%m", example: "2026/12", desc: "Year / Month" },
  ];

  /** @param {string} p */
  function selectDatePattern(p) {
    preferences.date_folders = p;
  }

  /** @param {number} delta */
  function adjustTarget(delta) {
    const current = Number(preferences.ai_cull_target) || 24;
    preferences.ai_cull_target = Math.max(1, Math.min(200, current + delta));
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

  /**
   * Truncates a file path smartly to fit the UI pill
   * @param {string} path
   * @returns {string}
   */
  function formatPath(path) {
    if (!path) return "";
    const parts = path.split("/").filter(Boolean);
    if (parts.length <= 2) return path;
    return `…/${parts.slice(-2).join("/")}`;
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="settings-window">
  <nav class="categories" data-tauri-drag-region>
    <div class="categories-spacer" data-tauri-drag-region></div>
    {#each visibleCategories as cat (cat.id)}
      <button
        class="category-btn"
        class:active={activeCategory === cat.id}
        onclick={() => (activeCategory = cat.id)}
      >
        <Icon name={cat.icon} size="14px" />
        <span>{cat.label}</span>
      </button>
    {/each}
  </nav>

  <div class="detail">
    <div class="detail-scroll">
      {#if activeCategory === "photos"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="image" size="12px" />
            <span>PHOTO ORGANIZATION</span>
          </div>
          <div class="inset-card date-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">DATE FOLDER STRUCTURE</span>
                <span class="row-desc">strftime pattern for the folder tree created on import.</span>
              </div>
              <div class="row-control date-control-col">
                <div class="input-with-presets">
                  <input
                    class="mono-input"
                    bind:value={preferences.date_folders}
                    placeholder="%Y/%Y-%m-%d"
                    spellcheck="false"
                  />
                  <Dropdown label="Date folder presets" triggerClass="ghost icon preset-toggle-btn" align="end">
                    {#snippet trigger()}<Icon name="caret-down" size="10px" />{/snippet}
                    {#each datePresets as preset}
                      <DropdownItem onclick={() => selectDatePattern(preset.pattern)}>
                        <span class="preset-option">
                          <span class="preset-code">{preset.pattern}</span>
                          <span class="preset-desc">{preset.desc}</span>
                          <span class="preset-example">{preset.example}</span>
                        </span>
                      </DropdownItem>
                    {/each}
                  </Dropdown>
                </div>
              </div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">AUTO-IMPORT</span>
                <span class="row-desc">Automatically imports detected memory cards, without confirmation.</span>
              </div>
              <div class="row-control">
                <input
                  type="checkbox"
                  role="switch"
                  checked={autoImportEnabled}
                  onchange={(e) => onToggleAutoImport(/** @type {HTMLInputElement} */ (e.currentTarget).checked)}
                />
              </div>
            </div>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">DEFAULT DEVELOP ENGINE</span>
                <span class="row-desc">Engine applied to photos that don't have settings yet — a freshly arrived import.</span>
              </div>
              <div class="row-control">
                <Dropdown label="Default engine" triggerClass="outline small action-pill-btn" align="end">
                  {#snippet trigger()}
                    <span>{engines.find((e) => e.id === preferences.default_engine)?.label ?? "None"}</span>
                    <Icon name="caret-down" size="10px" />
                  {/snippet}
                  <DropdownItem onclick={() => (preferences.default_engine = "")}>None</DropdownItem>
                  {#each engines as engine}
                    <DropdownItem onclick={() => (preferences.default_engine = engine.id)}>{engine.label}</DropdownItem>
                  {/each}
                </Dropdown>
              </div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">DEFAULT IMPORT PRESET</span>
                <span class="row-desc">Applied automatically to every photo that arrives from a memory card.</span>
              </div>
              <div class="row-control">
                <Dropdown label="Default import preset" triggerClass="outline small action-pill-btn" align="end">
                  {#snippet trigger()}
                    <span>{defaultImportPreset ?? "None"}</span>
                    <Icon name="caret-down" size="10px" />
                  {/snippet}
                  <DropdownItem onclick={() => onSetDefaultImportPreset(null)}>None</DropdownItem>
                  {#each presets as preset}
                    <DropdownItem onclick={() => onSetDefaultImportPreset(preset.name)}>{preset.name}</DropdownItem>
                  {/each}
                </Dropdown>
              </div>
            </div>
          </div>
        </div>
      {:else if activeCategory === "appearance"}
        <div class="section-group">
          <div class="inset-card date-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">THEME</span>
                <span class="row-desc">Applies right away, in every open window — no need to Save.</span>
              </div>
              <div class="row-control">
                <Dropdown label="Theme" triggerClass="outline small action-pill-btn" align="end">
                  {#snippet trigger()}
                    <span>{themes.find((t) => t.id === preferences.app_theme)?.label ?? "Reveal"}</span>
                    <Icon name="caret-down" size="10px" />
                  {/snippet}
                  {#each themes as theme}
                    <DropdownItem onclick={() => onSelectTheme(theme.id)}>{theme.label}</DropdownItem>
                  {/each}
                </Dropdown>
              </div>
            </div>
          </div>
        </div>
      {:else if activeCategory === "locations"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="folder-open" size="12px" />
            <span>LOCATIONS &amp; EXPORT</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">DEFAULT EXPORT FOLDER</span>
                <span class="row-desc">Destination for developed JPEGs outside the vault. Defaults to the Desktop.</span>
              </div>
              <div class="row-control">
                <div class="path-picker-group">
                  <div class="path-display" title={preferences.export_folder || "Desktop (default)"}>
                    <Icon name="folder-open" size="12px" />
                    <span class="path-text mono">
                      {preferences.export_folder ? formatPath(preferences.export_folder) : "Desktop (default)"}
                    </span>
                    {#if preferences.export_folder}
                      <button
                        type="button"
                        class="ghost icon clear-path-btn"
                        onclick={() => (preferences.export_folder = "")}
                        title="Reset to Desktop"
                      >
                        <Icon name="x" size="10px" />
                      </button>
                    {/if}
                  </div>
                  <button type="button" class="outline small action-pill-btn" onclick={() => onChooseFolder("export_folder")}>
                    CHOOSE…
                  </button>
                </div>
              </div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">CUSTOM LUTS FOLDER</span>
                <span class="row-desc">Location of your .cube files for creative profiles and grades.</span>
              </div>
              <div class="row-control">
                <div class="path-picker-group">
                  <div class="path-display" title={preferences.lut_folder || "Built-in LUTs only"}>
                    <Icon name="folder-open" size="12px" />
                    <span class="path-text mono">
                      {preferences.lut_folder ? formatPath(preferences.lut_folder) : "Built-in LUTs"}
                    </span>
                    {#if preferences.lut_folder}
                      <button
                        type="button"
                        class="ghost icon clear-path-btn"
                        onclick={() => (preferences.lut_folder = "")}
                        title="Clear"
                      >
                        <Icon name="x" size="10px" />
                      </button>
                    {/if}
                  </div>
                  <button type="button" class="outline small action-pill-btn" onclick={() => onChooseFolder("lut_folder")}>
                    CHOOSE…
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      {:else if activeCategory === "obsidian"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="note-pencil" size="12px" />
            <span>OBSIDIAN INTEGRATION</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">ENABLE OBSIDIAN INTEGRATION</span>
                <span class="row-desc">Lets you add photos to Daily Notes and write into an Obsidian vault.</span>
              </div>
              <div class="row-control">
                <input type="checkbox" role="switch" bind:checked={preferences.obsidian_enabled} />
              </div>
            </div>
            {#if preferences.obsidian_enabled}
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">OBSIDIAN VAULT ROOT</span>
                  <span class="row-desc">Root folder for writing catalog notes and daily notes.</span>
                </div>
                <div class="row-control">
                  <div class="path-picker-group">
                    <div class="path-display" title={preferences.vault || "~/Documents/Atelier (default)"}>
                      <Icon name="folder-open" size="12px" />
                      <span class="path-text mono">
                        {preferences.vault ? formatPath(preferences.vault) : "~/Documents/Atelier (default)"}
                      </span>
                      {#if preferences.vault}
                        <button
                          type="button"
                          class="ghost icon clear-path-btn"
                          onclick={() => (preferences.vault = "")}
                          title="Reset to default"
                        >
                          <Icon name="x" size="10px" />
                        </button>
                      {/if}
                    </div>
                    <button type="button" class="outline small action-pill-btn" onclick={() => onChooseFolder("vault")}>
                      CHOOSE…
                    </button>
                  </div>
                </div>
              </div>
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">DAILY NOTES SUBFOLDER</span>
                  <span class="row-desc">Relative location in the vault (e.g. Logs, Journal, Logs/Daily).</span>
                </div>
                <div class="row-control">
                  <input class="mono-input" bind:value={preferences.logs_folder} placeholder="Logs" spellcheck="false" />
                </div>
              </div>
            {/if}
          </div>
        </div>
      {:else if activeCategory === "library"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="books" size="12px" />
            <span>CATALOGUED LIBRARIES</span>
          </div>
          <div class="inset-card">
            {#if libraries === null}
              <div class="setting-row"><span class="row-desc">Reading libraries…</span></div>
            {:else if libraries.length === 0}
              <div class="setting-row">
                <span class="row-desc">No library yet. Add a folder of photos to catalogue it.</span>
              </div>
            {:else}
              {#each libraries as lib (lib.path)}
                <div class="setting-row">
                  <div class="row-meta">
                    <span class="row-label">{lib.path.split("/").pop() || lib.path}</span>
                    <span class="row-desc lib-path">{lib.path}</span>
                    <span class="row-desc">
                      {lib.frames.toLocaleString("en-CA")} photo{lib.frames === 1 ? "" : "s"}
                      {#if !lib.online}
                        · <span class="lib-offline">offline — the folder isn't reachable right now</span>
                      {/if}
                    </span>
                  </div>
                  <div class="row-control cache-actions">
                    <button
                      type="button"
                      class="outline small action-pill-btn"
                      disabled={libBusy || !lib.online}
                      title={lib.online ? "Rescan this library" : "Unavailable while the folder is offline"}
                      onclick={() => rescanLibrary(lib.path)}
                    >Reindex</button>
                    <button
                      type="button"
                      class="outline small action-pill-btn"
                      disabled={libBusy}
                      onclick={() => { libToRemove = lib; }}
                    >Remove…</button>
                  </div>
                </div>
              {/each}
            {/if}
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">ADD A LIBRARY</span>
                <!-- Said plainly because an offline NAS looks exactly like a
                     deleted folder, and "Remove" is the button next to it. -->
                <span class="row-desc">
                  Removing a library only forgets it here — no photo is ever deleted from disk.
                  The ratings, captions and story marks the catalogue holds for it do go, and
                  come back only by reindexing.
                </span>
              </div>
              <div class="row-control">
                <button type="button" class="outline small action-pill-btn" disabled={libBusy} onclick={addLibrary}>
                  Add folder…
                </button>
              </div>
            </div>
          </div>
          {#if libError}<div role="alert"><Alert class="error">{libError}</Alert></div>{/if}
        </div>

      {:else if activeCategory === "cache"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="hard-drive" size="12px" />
            <span>DEVELOPED PREVIEWS</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">JPEG RENDER CACHE</span>
                <span class="row-desc">Developed copies cached for instant display in the grid — independent of the source (card, Apple Photos, disk). Clearing forces a redevelop on next display.</span>
                <span class="row-desc" role="status" aria-label="Preview cache usage">
                  {#if devCacheStatus}
                    {formatBytes(devCacheStatus.size_bytes)} / {formatBytes(devCacheStatus.limit_bytes)} · {devCacheStatus.photo_count.toLocaleString("en-CA")} photos
                  {:else}
                    {devCacheBusy ? "Reading cache usage…" : "Cache usage unavailable."}
                  {/if}
                </span>
              </div>
              <div class="row-control cache-actions">
                <button type="button" class="outline small action-pill-btn" onclick={loadDevCacheStatus} disabled={devCacheBusy}>Refresh</button>
                <button type="button" class="outline small action-pill-btn" onclick={() => { devCacheConfirm = true; }} disabled={devCacheBusy}>Clear cache…</button>
              </div>
            </div>
          </div>
          {#if devCacheError}<div role="alert"><Alert class="error">{devCacheError}</Alert></div>{/if}
          {#if devCacheMessage}<div role="status"><Alert class="info">{devCacheMessage}</Alert></div>{/if}
        </div>
        {#if cacheAvailable}
          <div class="section-group">
            <div class="section-heading">
              <Icon name="image" size="12px" />
              <span>APPLE PHOTOS CACHE</span>
            </div>
            <div class="inset-card">
              <div class="setting-row">
                <div class="row-meta">
                  <label class="row-label" for="photos-cache-limit">CACHE LIMIT (GiB)</label>
                  <span class="row-desc">1–64 GiB, applied when settings are saved. Files in use remain protected.</span>
                </div>
                <div class="row-control">
                  <input id="photos-cache-limit" class="mono-input" type="number" min="1" max="64" step="1"
                    bind:value={preferences.apple_photos_cache_limit_gib} disabled={saving} />
                </div>
              </div>
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">CACHED WORKING COPIES</span>
                  <span class="row-desc">Clearing is safe for edits, ratings and captions. Originals and previews may need downloading again.</span>
                  <span class="row-desc" role="status" aria-label="Cache usage">
                    {#if cacheStatus}
                      {formatBytes(cacheStatus.size_bytes)} used / {formatBytes(cacheStatus.limit_bytes)} saved limit.
                      {formatBytes(cacheStatus.in_use_bytes)} currently in use.
                    {:else}
                      {cacheBusy ? "Reading cache usage…" : "Cache usage unavailable."}
                    {/if}
                  </span>
                </div>
                <div class="row-control cache-actions">
                  <button type="button" class="outline small action-pill-btn" onclick={loadCacheStatus} disabled={cacheBusy || saving}>Refresh usage</button>
                  <button type="button" class="outline small action-pill-btn" onclick={() => { cacheConfirm = true; }} disabled={cacheBusy || saving}>Clear cached copies…</button>
                </div>
              </div>
            </div>
            {#if cacheError}<div role="alert"><Alert class="error">{cacheError}</Alert></div>{/if}
            {#if cacheMessage}<div role="status"><Alert class="info">{cacheMessage}</Alert></div>{/if}
          </div>
        {/if}
      {:else if activeCategory === "garden"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="user-circle" size="12px" />
            <span>GARDEN ACCOUNT</span>
          </div>
          <div class="inset-card">
            {#if gardenAccount?.signed_in}
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">{gardenAccount.username ?? "Connected"}</span>
                  <span class="row-desc">Tier: {gardenAccount.tier ?? "—"}</span>
                </div>
                <div class="row-control">
                  <button type="button" class="outline small action-pill-btn" onclick={signOutOfGarden} disabled={gardenSigningOut}>
                    {gardenSigningOut ? "Signing out…" : "Sign out"}
                  </button>
                </div>
              </div>
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">PUBLISHED NOTES</span>
                </div>
                <div class="row-control"><span class="mono">{gardenAccount.notes_count ?? 0}</span></div>
              </div>
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">TOTAL VIEWS</span>
                </div>
                <div class="row-control"><span class="mono">{(gardenAccount.total_views ?? 0).toLocaleString("en-CA")}</span></div>
              </div>
            {:else}
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">NOT SIGNED IN</span>
                  <span class="row-desc">Sign in from the sidebar (Garden button) to publish your stories.</span>
                </div>
              </div>
            {/if}
          </div>
          {#if gardenError}<div role="alert"><Alert class="error">{gardenError}</Alert></div>{/if}
        </div>
      {:else if activeCategory === "ai"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="lightning" size="12px" />
            <span>AI CULLING &amp; AUTOMATION</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">AUTO-SELECT (STORY)</span>
                <span class="row-desc">Adds the best photos to the quick collection (like the Q key) after every import — no stars, just ready in the preview.</span>
              </div>
              <div class="row-control">
                <input type="checkbox" role="switch" bind:checked={preferences.ai_cull_mark_story} />
              </div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">AUTO-EXPORT (DESKTOP)</span>
                <span class="row-desc">Develops and exports the best photos to a Desktop folder after every import.</span>
              </div>
              <div class="row-control">
                <input type="checkbox" role="switch" bind:checked={preferences.ai_cull_export_desktop} />
              </div>
            </div>
            <div class="setting-row" class:row-disabled={!aiCullActive}>
              <div class="row-meta">
                <span class="row-label">PHOTOS TO KEEP PER SESSION</span>
                <span class="row-desc">Target number of photos kept per date folder during auto-cull — the 24-or-36-exposure roll concept.</span>
              </div>
              <div class="row-control">
                <div class="stepper-group">
                  <button type="button" class="stepper-btn" disabled={!aiCullActive || preferences.ai_cull_target <= 1} onclick={() => adjustTarget(-1)} title="Decrease">−</button>
                  <input type="number" class="stepper-input mono" min="1" max="200" disabled={!aiCullActive} bind:value={preferences.ai_cull_target} />
                  <button type="button" class="stepper-btn" disabled={!aiCullActive || preferences.ai_cull_target >= 200} onclick={() => adjustTarget(1)} title="Increase">+</button>
                </div>
              </div>
            </div>
            <div class="setting-row" class:row-disabled={!aiCullActive}>
              <div class="row-meta">
                <span class="row-label">VISION PROVIDER</span>
                <span class="row-desc">Who scores culling candidates and suggests photo tags. Same provider for both — swap it here, not per-feature.</span>
              </div>
              <div class="row-control">
                <Dropdown label="Vision provider" triggerClass="outline small action-pill-btn" align="end">
                  {#snippet trigger()}
                    <span>{AI_PROVIDERS.find((p) => p.id === preferences.ai_provider)?.label ?? "Anthropic (Claude)"}</span>
                    <Icon name="caret-down" size="10px" />
                  {/snippet}
                  {#each AI_PROVIDERS as provider}
                    <DropdownItem onclick={() => (preferences.ai_provider = provider.id)}>{provider.label}</DropdownItem>
                  {/each}
                </Dropdown>
              </div>
            </div>
            <div class="setting-row" class:row-disabled={!aiCullActive}>
              <div class="row-meta">
                <span class="row-label">VISION API KEY</span>
                <span class="row-desc">Sends compressed thumbnails to the provider above for ranking and tag suggestions. Billed per API usage.</span>
              </div>
              <div class="row-control">
                <input type="password" class="mono-input" disabled={!aiCullActive} bind:value={preferences.ai_api_key} placeholder={preferences.ai_provider === "gemini" ? "AIza…" : "sk-ant-…"} autocomplete="off" spellcheck="false" />
              </div>
            </div>
            <div class="setting-row" class:row-disabled={!aiCullActive}>
              <div class="row-meta">
                <span class="row-label">MODEL</span>
                <span class="row-desc">Leave blank for the provider's default. Must be a valid model id for the selected provider, or requests will fail.</span>
              </div>
              <div class="row-control">
                <input class="mono-input" disabled={!aiCullActive} bind:value={preferences.ai_model} placeholder={AI_PROVIDERS.find((p) => p.id === preferences.ai_provider)?.modelPlaceholder ?? "claude-sonnet-5"} autocomplete="off" spellcheck="false" />
              </div>
            </div>
          </div>
        </div>
      {:else if activeCategory === "about"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="info" size="12px" />
            <span>REVEAL</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">VERSION</span>
              </div>
              <div class="row-control"><span class="mono">{appVersion || "—"}</span></div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-desc">Free and open source — a personal darkroom, not a product. Built on the same appetite for crediting the work it stands on that it asks of anyone using it.</span>
              </div>
            </div>
          </div>
        </div>
        <div class="section-group">
          <div class="section-heading">
            <Icon name="heart" size="12px" />
            <span>OPEN SOURCE &amp; CREDITS</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">SPEKTRAFILM-RS</span>
                <span class="row-desc">The film-emulation engine behind the Spektra develop mode — turbasvin's Rust port of spektrafilm, pinned per release.</span>
              </div>
              <div class="row-control">
                <button type="button" class="outline small action-pill-btn" onclick={() => openExternal("https://github.com/turbasvin/spektrafilm-rs")}>
                  <Icon name="arrow-square-out" size="10px" />
                  <span>GitHub</span>
                </button>
              </div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">RAPIDRAW</span>
                <span class="row-desc">Timon Käch's GPU-accelerated RAW editor (AGPL-3.0) — a source of real inspiration for where Reveal's own develop engine can go.</span>
              </div>
              <div class="row-control">
                <button type="button" class="outline small action-pill-btn" onclick={() => openExternal("https://github.com/CyberTimon/RapidRAW")}>
                  <Icon name="arrow-square-out" size="10px" />
                  <span>GitHub</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      {/if}
    </div>

    {#if saveError}<div class="save-error" role="alert"><Alert class="error">{saveError}</Alert></div>{/if}
    <footer class="settings-footer">
      <div class="footer-actions">
        <button type="button" class="outline small btn-cancel" onclick={() => onClose()}>CANCEL</button>
        <button type="button" class="secondary small btn-save" onclick={save} disabled={saving || cacheBusy}>SAVE</button>
      </div>
    </footer>
  </div>
</div>

<AlertDialog bind:open={cacheConfirm} title="Clear Apple Photos cached copies?"
  description="Only downloaded working copies and previews will be cleared. Your edits, ratings and captions are safe. Files currently in use will be kept; other originals may need downloading again."
  confirmLabel="Clear cached copies" cancelLabel="Keep cached copies" intent="danger"
  onconfirm={clearCache} oncancel={() => { cacheConfirm = false; }} />

<AlertDialog
  open={!!libToRemove}
  title={libToRemove ? `Remove “${libToRemove.path.split("/").pop()}” from Reveal?` : ""}
  description={libToRemove
    ? `No photo is deleted — the ${libToRemove.frames.toLocaleString("en-CA")} files stay exactly where they are on disk. Reveal forgets this library, along with the ratings, captions and story marks its catalogue holds for them. Adding the folder back and reindexing restores the photos, not those marks.`
    : ""}
  confirmLabel="Remove library" cancelLabel="Keep it" intent="danger"
  onconfirm={confirmRemoveLibrary} oncancel={() => { libToRemove = null; }} />

<AlertDialog bind:open={devCacheConfirm} title="Clear developed preview cache?"
  description="Rendered JPEG copies will be deleted. Your settings, notes and stars are saved elsewhere and unaffected — photos simply redevelop on next display."
  confirmLabel="Clear cache" cancelLabel="Keep cache" intent="danger"
  onconfirm={clearDevCache} oncancel={() => { devCacheConfirm = false; }} />

<style>
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    height: 100%;
    /* The same ground as the main window's photo area: the settings sit
       on the recessed canvas, the categories float above it. */
    background: var(--canvas);
    color: var(--color-foreground);
    overflow: hidden;
  }

  .settings-window {
    height: 100vh;
    display: flex;
  }

  /* Categories — the same floating pane as the main window's sidebar
     (Sidebar.svelte's nav): inset, concentric corner, raised. The traffic
     lights overlay its top, as they do the sidebar's. */
  .categories {
    width: 190px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: var(--window-inset);
    padding: 0 8px 8px;
    background: var(--color-surface-high);
    border: var(--border);
    border-radius: var(--pane-radius);
    box-shadow: var(--shadow-raised);
    overflow-y: auto;
  }
  /* Clears the traffic lights: the title-bar band, less the pane's own
     inset from the window top. */
  .categories-spacer {
    height: calc(var(--titlebar-height) - var(--window-inset));
    flex-shrink: 0;
  }
  .category-btn {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border-radius: var(--radius);
    font-family: var(--font-text, sans-serif);
    font-size: 12px;
    color: color-mix(in srgb, var(--color-foreground) 60%, transparent);
    cursor: pointer;
  }
  /* Selection reads exactly like a folder row in the sidebar: a quiet
     foreground wash and full-strength text, not an accent block. */
  .category-btn:hover {
    background: color-mix(in srgb, var(--color-foreground) 6%, transparent);
    color: var(--color-foreground);
  }
  .category-btn.active {
    background: color-mix(in srgb, var(--color-foreground) 12%, transparent);
    color: var(--color-foreground);
    font-weight: 600;
  }

  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .detail-scroll {
    padding: var(--titlebar-height) 1.5rem 1.25rem;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }
  .save-error {
    padding: 0 1.5rem;
  }

  .cache-actions { flex-wrap: wrap; gap: var(--space-half); }

  .section-group {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .section-heading {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding-left: 0.25rem;
    font-family: var(--font-header, sans-serif);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.1em;
    color: color-mix(in srgb, var(--color-foreground, #fff) 50%, transparent);
    text-transform: uppercase;
  }

  .inset-card {
    background: color-mix(in srgb, var(--color-foreground, #fff) 3.5%, var(--color-surface-low, #18181b));
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 8%, transparent);
    border-radius: var(--radius, 8px);
    overflow: hidden;
  }
  /* .inset-card's overflow:hidden clips any Dropdown popover open inside it
     to the card's own bounds — used on any card whose row has one (the date
     preset picker, the theme picker), not literally date-specific anymore. */
  .date-card { overflow: visible; }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 0.9rem;
    gap: 1rem;
    transition: background 120ms ease, opacity 150ms ease;
  }
  .setting-row:not(:last-child) {
    border-bottom: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 5%, transparent);
  }
  .setting-row:hover {
    background: color-mix(in srgb, var(--color-foreground, #fff) 1.5%, transparent);
  }
  .setting-row.row-disabled {
    opacity: 0.4;
    pointer-events: none;
  }

  .row-meta {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    flex: 1;
    min-width: 0;
  }
  .row-label {
    font-family: var(--font-header, sans-serif);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--color-foreground, #f4f4f5);
  }
  .lib-path {
    font-family: var(--font-monospace, monospace);
    font-size: 0.62rem;
    opacity: 0.6;
    overflow-wrap: anywhere;
  }
  .lib-offline {
    color: var(--color-accent);
  }
    .row-desc {
    font-family: var(--font-text, system-ui, sans-serif);
    font-size: 11px;
    line-height: 1.35;
    color: color-mix(in srgb, var(--color-foreground, #fff) 48%, transparent);
  }
  .row-control {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
  }

  /* Sizing only — border/background/focus ring come from Standard's own
     zero-class input rules (_standard-11-forms.scss); this panel just needs
     a narrower, monospace-set field than the 1em default. */
  .mono-input {
    box-sizing: border-box;
    padding: 0.35rem 0.65rem;
    width: 190px;
    max-width: 100%;
    font-family: var(--font-monospace, monospace);
    font-size: 11px;
  }

  .date-control-col {
    position: relative;
  }
  .input-with-presets {
    display: flex;
    align-items: center;
    position: relative;
  }
  .input-with-presets .mono-input {
    padding-right: 26px;
    width: 190px;
  }
  .input-with-presets :global(.preset-toggle-btn) {
    width: 20px;
    height: 20px;
    padding: 0;
  }

  .preset-option {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px 0;
  }
  .preset-code {
    font-family: var(--font-monospace, monospace);
    font-size: 11px;
    font-weight: 600;
    color: var(--color-foreground, #fff);
  }
  .preset-desc {
    font-family: var(--font-text, sans-serif);
    font-size: 10px;
    color: color-mix(in srgb, var(--color-foreground, #fff) 55%, transparent);
  }
  .preset-example {
    font-family: var(--font-monospace, monospace);
    font-size: 9.5px;
    color: color-mix(in srgb, var(--color-accent, #d6202c) 85%, white);
  }

  .path-picker-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .path-display {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0.35rem 0.6rem;
    background: color-mix(in srgb, var(--color-foreground, #fff) 3%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 9%, transparent);
    border-radius: var(--radius-sm, 4px);
    max-width: 180px;
    color: color-mix(in srgb, var(--color-foreground, #fff) 80%, transparent);
  }
  .path-text {
    font-size: 10.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Sizing only from here down — colors, borders, hover/active states, and
     the pill shape all come from Standard's button rules
     (_standard-13-components.scss: plain button, .outline, .secondary,
     .small, .icon) plus the app-wide pill shape in +layout.svelte. */
  .clear-path-btn {
    width: 18px;
    height: 18px;
    padding: 0;
  }

  .action-pill-btn {
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.08em;
    white-space: nowrap;
    gap: 4px;
  }

  .stepper-group {
    display: flex;
    align-items: center;
    background: color-mix(in srgb, var(--color-foreground, #fff) 4%, var(--color-surface-low, #18181b));
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 12%, transparent);
    border-radius: var(--radius-sm, 4px);
    overflow: hidden;
  }
  .stepper-btn {
    all: unset;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    color: var(--color-foreground, #fff);
    background: color-mix(in srgb, var(--color-foreground, #fff) 4%, transparent);
    user-select: none;
    transition: background 120ms ease;
  }
  .stepper-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-foreground, #fff) 12%, transparent);
  }
  .stepper-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .stepper-input {
    width: 44px;
    height: 26px;
    text-align: center;
    border: none;
    background: transparent;
    color: var(--color-foreground, #fff);
    font-family: var(--font-monospace, monospace);
    font-size: 11.5px;
    font-weight: 600;
    outline: none;
    -moz-appearance: textfield;
    appearance: textfield;
  }
  .stepper-input::-webkit-outer-spin-button,
  .stepper-input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .settings-footer {
    padding: 0.85rem 1.25rem;
    border-top: var(--border);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    user-select: none;
  }
  .footer-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .btn-cancel,
  .btn-save {
    font-family: var(--font-header, sans-serif);
    font-size: 11px;
    letter-spacing: 0.08em;
  }
  .mono {
    font-family: var(--font-monospace, monospace);
  }
</style>
