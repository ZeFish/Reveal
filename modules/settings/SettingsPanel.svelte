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
    /** @type {() => Promise<{size_bytes: number, photo_count: number, limit_photos: number}>} */
    onPreviewCacheStatus = async () => ({ size_bytes: 0, photo_count: 0, limit_photos: 0 }),
    /** @type {() => Promise<{removed_bytes: number}>} */
    onPreviewCacheClear = async () => ({ removed_bytes: 0 }),
    /** @type {GardenAccount | null} */
    gardenAccount = null,
    /** @type {() => Promise<void>} */
    onGardenSignOut = async () => {},
    /** @type {EngineInfo[]} */
    engines = [],
  } = $props();

  const CATEGORIES = [
    { id: "photos", label: "Photos", icon: "image" },
    { id: "appearance", label: "Apparence", icon: "palette" },
    { id: "locations", label: "Emplacements", icon: "folder-open" },
    { id: "obsidian", label: "Obsidian", icon: "note-pencil" },
    { id: "cache", label: "Cache & Stockage", icon: "hard-drive" },
    { id: "garden", label: "Compte Garden", icon: "user-circle" },
    { id: "ai", label: "IA & Automatisation", icon: "lightning" },
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
  let devCacheError = $state("");
  let devCacheMessage = $state("");
  /** @type {{size_bytes: number, photo_count: number, limit_photos: number} | null} */
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
    { pattern: "%Y/%Y-%m-%d", example: "2026/2026-12-31", desc: "Année / Année-Mois-Jour" },
    { pattern: "%Y/%m/%d", example: "2026/12/31", desc: "Année / Mois / Jour" },
    { pattern: "%Y-%m-%d", example: "2026-12-31", desc: "Année-Mois-Jour à plat" },
    { pattern: "%Y/%m", example: "2026/12", desc: "Année / Mois" },
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
            <span>ORGANISATION DES PHOTOS</span>
          </div>
          <div class="inset-card date-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">STRUCTURE DES DOSSIERS DE DATE</span>
                <span class="row-desc">Modèle strftime pour l'arborescence des dossiers créés à l'import.</span>
              </div>
              <div class="row-control date-control-col">
                <div class="input-with-presets">
                  <input
                    class="mono-input"
                    bind:value={preferences.date_folders}
                    placeholder="%Y/%Y-%m-%d"
                    spellcheck="false"
                  />
                  <Dropdown label="Date folder presets" triggerClass="preset-toggle-btn" align="end">
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
                <span class="row-label">IMPORT AUTOMATIQUE</span>
                <span class="row-desc">Importe automatiquement les cartes mémoire détectées, sans confirmation.</span>
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
                <span class="row-label">MOTEUR DE DÉVELOPPEMENT PAR DÉFAUT</span>
                <span class="row-desc">Moteur appliqué aux photos qui n'ont pas encore de réglages — un import fraîchement arrivé.</span>
              </div>
              <div class="row-control">
                <Dropdown label="Default engine" triggerClass="action-pill-btn" align="end">
                  {#snippet trigger()}
                    <span>{engines.find((e) => e.id === preferences.default_engine)?.label ?? "Aucun"}</span>
                    <Icon name="caret-down" size="10px" />
                  {/snippet}
                  <DropdownItem onclick={() => (preferences.default_engine = "")}>Aucun</DropdownItem>
                  {#each engines as engine}
                    <DropdownItem onclick={() => (preferences.default_engine = engine.id)}>{engine.label}</DropdownItem>
                  {/each}
                </Dropdown>
              </div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">PRESET PAR DÉFAUT À L'IMPORT</span>
                <span class="row-desc">Appliqué automatiquement à chaque photo qui arrive d'une carte mémoire.</span>
              </div>
              <div class="row-control">
                <Dropdown label="Default import preset" triggerClass="action-pill-btn" align="end">
                  {#snippet trigger()}
                    <span>{defaultImportPreset ?? "Aucun"}</span>
                    <Icon name="caret-down" size="10px" />
                  {/snippet}
                  <DropdownItem onclick={() => onSetDefaultImportPreset(null)}>Aucun</DropdownItem>
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
          <div class="section-heading">
            <Icon name="palette" size="12px" />
            <span>THÈME</span>
          </div>
          <div class="inset-card date-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">IDENTITÉ VISUELLE</span>
                <span class="row-desc">S'applique tout de suite, dans toutes les fenêtres ouvertes — pas besoin d'Enregistrer.</span>
              </div>
              <div class="row-control">
                <Dropdown label="Theme" triggerClass="action-pill-btn" align="end">
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
            <span>EMPLACEMENTS &amp; EXPORT</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">DOSSIER D'EXPORT PAR DÉFAUT</span>
                <span class="row-desc">Destination des JPEG développés hors du vault. Par défaut : le Bureau.</span>
              </div>
              <div class="row-control">
                <div class="path-picker-group">
                  <div class="path-display" title={preferences.export_folder || "Bureau (par défaut)"}>
                    <Icon name="folder-open" size="12px" />
                    <span class="path-text mono">
                      {preferences.export_folder ? formatPath(preferences.export_folder) : "Bureau (par défaut)"}
                    </span>
                    {#if preferences.export_folder}
                      <button
                        type="button"
                        class="clear-path-btn"
                        onclick={() => (preferences.export_folder = "")}
                        title="Réinitialiser au Bureau"
                      >
                        <Icon name="x" size="10px" />
                      </button>
                    {/if}
                  </div>
                  <button type="button" class="action-pill-btn" onclick={() => onChooseFolder("export_folder")}>
                    CHOISIR…
                  </button>
                </div>
              </div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">DOSSIER DES LUTS PERSONNALISÉES</span>
                <span class="row-desc">Emplacement de vos fichiers .cube pour les profils et étalonnages créatifs.</span>
              </div>
              <div class="row-control">
                <div class="path-picker-group">
                  <div class="path-display" title={preferences.lut_folder || "LUTs intégrées uniquement"}>
                    <Icon name="folder-open" size="12px" />
                    <span class="path-text mono">
                      {preferences.lut_folder ? formatPath(preferences.lut_folder) : "LUTs intégrées"}
                    </span>
                    {#if preferences.lut_folder}
                      <button
                        type="button"
                        class="clear-path-btn"
                        onclick={() => (preferences.lut_folder = "")}
                        title="Effacer"
                      >
                        <Icon name="x" size="10px" />
                      </button>
                    {/if}
                  </div>
                  <button type="button" class="action-pill-btn" onclick={() => onChooseFolder("lut_folder")}>
                    CHOISIR…
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
            <span>INTÉGRATION OBSIDIAN</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">ACTIVER L'INTÉGRATION OBSIDIAN</span>
                <span class="row-desc">Permet d'ajouter des photos au journal quotidien (Daily Notes) et d'écrire dans un coffre Obsidian.</span>
              </div>
              <div class="row-control">
                <input type="checkbox" role="switch" bind:checked={preferences.obsidian_enabled} />
              </div>
            </div>
            {#if preferences.obsidian_enabled}
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">RACINE DU VAULT OBSIDIAN</span>
                  <span class="row-desc">Dossier racine pour l'écriture des fiches catalogues et notes journalières.</span>
                </div>
                <div class="row-control">
                  <div class="path-picker-group">
                    <div class="path-display" title={preferences.vault || "~/Documents/Atelier (par défaut)"}>
                      <Icon name="folder-open" size="12px" />
                      <span class="path-text mono">
                        {preferences.vault ? formatPath(preferences.vault) : "~/Documents/Atelier (par défaut)"}
                      </span>
                      {#if preferences.vault}
                        <button
                          type="button"
                          class="clear-path-btn"
                          onclick={() => (preferences.vault = "")}
                          title="Réinitialiser par défaut"
                        >
                          <Icon name="x" size="10px" />
                        </button>
                      {/if}
                    </div>
                    <button type="button" class="action-pill-btn" onclick={() => onChooseFolder("vault")}>
                      CHOISIR…
                    </button>
                  </div>
                </div>
              </div>
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">SOUS-DOSSIER DES NOTES QUOTIDIENNES</span>
                  <span class="row-desc">Emplacement relatif dans le vault (ex : Logs, Journal, Logs/Daily).</span>
                </div>
                <div class="row-control">
                  <input class="mono-input" bind:value={preferences.logs_folder} placeholder="Logs" spellcheck="false" />
                </div>
              </div>
            {/if}
          </div>
        </div>
      {:else if activeCategory === "cache"}
        <div class="section-group">
          <div class="section-heading">
            <Icon name="hard-drive" size="12px" />
            <span>APERÇUS DÉVELOPPÉS</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">CACHE DE RENDU JPEG</span>
                <span class="row-desc">Copies développées mises en cache pour un affichage instantané dans la grille — indépendant de la source (carte, Apple Photos, disque). Vider force un redéveloppement au prochain affichage.</span>
                <span class="row-desc" role="status" aria-label="Preview cache usage">
                  {#if devCacheStatus}
                    {formatBytes(devCacheStatus.size_bytes)} · {devCacheStatus.photo_count.toLocaleString("en-CA")} / {devCacheStatus.limit_photos.toLocaleString("en-CA")} photos
                  {:else}
                    {devCacheBusy ? "Lecture de l'usage du cache…" : "Usage du cache indisponible."}
                  {/if}
                </span>
              </div>
              <div class="row-control cache-actions">
                <button type="button" class="action-pill-btn" onclick={loadDevCacheStatus} disabled={devCacheBusy}>Rafraîchir</button>
                <button type="button" class="action-pill-btn" onclick={() => { devCacheConfirm = true; }} disabled={devCacheBusy}>Vider le cache…</button>
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
                  <button type="button" class="action-pill-btn" onclick={loadCacheStatus} disabled={cacheBusy || saving}>Refresh usage</button>
                  <button type="button" class="action-pill-btn" onclick={() => { cacheConfirm = true; }} disabled={cacheBusy || saving}>Clear cached copies…</button>
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
            <span>COMPTE GARDEN</span>
          </div>
          <div class="inset-card">
            {#if gardenAccount?.signed_in}
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">{gardenAccount.username ?? "Connecté"}</span>
                  <span class="row-desc">Palier : {gardenAccount.tier ?? "—"}</span>
                </div>
                <div class="row-control">
                  <button type="button" class="action-pill-btn" onclick={signOutOfGarden} disabled={gardenSigningOut}>
                    {gardenSigningOut ? "Déconnexion…" : "Se déconnecter"}
                  </button>
                </div>
              </div>
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">NOTES PUBLIÉES</span>
                </div>
                <div class="row-control"><span class="mono">{gardenAccount.notes_count ?? 0}</span></div>
              </div>
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">VUES TOTALES</span>
                </div>
                <div class="row-control"><span class="mono">{(gardenAccount.total_views ?? 0).toLocaleString("en-CA")}</span></div>
              </div>
            {:else}
              <div class="setting-row">
                <div class="row-meta">
                  <span class="row-label">NON CONNECTÉ</span>
                  <span class="row-desc">Connectez-vous depuis la barre latérale (bouton Garden) pour publier vos histoires.</span>
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
            <span>AI CULLING &amp; AUTOMATISATION</span>
          </div>
          <div class="inset-card">
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">SÉLECTION AUTOMATIQUE (HISTOIRE)</span>
                <span class="row-desc">Ajoute les meilleures photos à la collection rapide (comme la touche Q) après chaque import — sans étoiles, juste prêtes dans l'aperçu.</span>
              </div>
              <div class="row-control">
                <input type="checkbox" role="switch" bind:checked={preferences.ai_cull_mark_story} />
              </div>
            </div>
            <div class="setting-row">
              <div class="row-meta">
                <span class="row-label">EXPORT AUTOMATIQUE (BUREAU)</span>
                <span class="row-desc">Développe et exporte les meilleures photos vers un dossier sur le Bureau après chaque import.</span>
              </div>
              <div class="row-control">
                <input type="checkbox" role="switch" bind:checked={preferences.ai_cull_export_desktop} />
              </div>
            </div>
            <div class="setting-row" class:row-disabled={!aiCullActive}>
              <div class="row-meta">
                <span class="row-label">PHOTOS À RETENIR PAR SESSION</span>
                <span class="row-desc">Nombre cible de photos retenues par dossier de date lors du tri automatique — le concept du 24 ou 36 poses.</span>
              </div>
              <div class="row-control">
                <div class="stepper-group">
                  <button type="button" class="stepper-btn" disabled={!aiCullActive || preferences.ai_cull_target <= 1} onclick={() => adjustTarget(-1)} title="Diminuer">−</button>
                  <input type="number" class="stepper-input mono" min="1" max="200" disabled={!aiCullActive} bind:value={preferences.ai_cull_target} />
                  <button type="button" class="stepper-btn" disabled={!aiCullActive || preferences.ai_cull_target >= 200} onclick={() => adjustTarget(1)} title="Augmenter">+</button>
                </div>
              </div>
            </div>
            <div class="setting-row" class:row-disabled={!aiCullActive}>
              <div class="row-meta">
                <span class="row-label">CLÉ API VISION (ANTHROPIC)</span>
                <span class="row-desc">Envoie des vignettes compressées à Claude (Sonnet 5 par défaut) pour classement. Facturé selon l'usage d'API.</span>
              </div>
              <div class="row-control">
                <input type="password" class="mono-input" disabled={!aiCullActive} bind:value={preferences.ai_api_key} placeholder="sk-ant-…" autocomplete="off" spellcheck="false" />
              </div>
            </div>
            <div class="setting-row" class:row-disabled={!aiCullActive}>
              <div class="row-meta">
                <span class="row-label">MODÈLE</span>
                <span class="row-desc">Laisser vide pour le défaut (claude-sonnet-5). Un identifiant de modèle Anthropic valide, sinon le culling échouera.</span>
              </div>
              <div class="row-control">
                <input class="mono-input" disabled={!aiCullActive} bind:value={preferences.ai_model} placeholder="claude-sonnet-5" autocomplete="off" spellcheck="false" />
              </div>
            </div>
          </div>
        </div>
      {/if}
    </div>

    {#if saveError}<div class="save-error" role="alert"><Alert class="error">{saveError}</Alert></div>{/if}
    <footer class="settings-footer">
      <span class="shortcut-tip"><kbd>⌘</kbd><kbd>Entrée</kbd> Enregistrer · <kbd>Échap</kbd> Fermer</span>
      <div class="footer-actions">
        <button type="button" class="btn-cancel" onclick={() => onClose()}>ANNULER</button>
        <button type="button" class="btn-save" onclick={save} disabled={saving || cacheBusy}>ENREGISTRER</button>
      </div>
    </footer>
  </div>
</div>

<AlertDialog bind:open={cacheConfirm} title="Clear Apple Photos cached copies?"
  description="Only downloaded working copies and previews will be cleared. Your edits, ratings and captions are safe. Files currently in use will be kept; other originals may need downloading again."
  confirmLabel="Clear cached copies" cancelLabel="Keep cached copies" intent="danger"
  onconfirm={clearCache} oncancel={() => { cacheConfirm = false; }} />

<AlertDialog bind:open={devCacheConfirm} title="Vider le cache des aperçus développés ?"
  description="Les copies JPEG rendues seront supprimées. Vos réglages, notes et étoiles sont sauvegardés ailleurs et ne sont pas affectés — les photos redéveloppent simplement au prochain affichage."
  confirmLabel="Vider le cache" cancelLabel="Garder le cache" intent="danger"
  onconfirm={clearDevCache} oncancel={() => { devCacheConfirm = false; }} />

<style>
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: var(--color-surface-high, #18181b);
    color: var(--color-foreground, #f4f4f5);
    overflow: hidden;
  }

  .settings-window {
    height: 100vh;
    display: flex;
  }

  /* Categories rail — macOS System Settings' left column. */
  .categories {
    width: 190px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    padding: 0 8px 8px;
    background: color-mix(in srgb, var(--color-foreground, #fff) 2%, var(--color-surface-low, #18181b));
    border-right: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 8%, transparent);
    overflow-y: auto;
  }
  .categories-spacer {
    height: 46px;
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
    margin-bottom: 2px;
    border-radius: var(--radius-sm, 6px);
    font-family: var(--font-text, sans-serif);
    font-size: 12px;
    color: color-mix(in srgb, var(--color-foreground, #fff) 75%, transparent);
    cursor: pointer;
  }
  .category-btn:hover {
    background: color-mix(in srgb, var(--color-foreground, #fff) 6%, transparent);
  }
  .category-btn.active {
    background: var(--color-accent, #d6202c);
    color: #fff;
  }

  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .detail-scroll {
    padding: 46px 1.5rem 1.25rem;
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

  .cache-actions { flex-wrap: wrap; gap: var(--space-xs); }

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
    gap: 0.2rem;
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

  .mono-input {
    box-sizing: border-box;
    padding: 0.35rem 0.65rem;
    width: 190px;
    max-width: 100%;
    background: color-mix(in srgb, var(--color-foreground, #fff) 4%, var(--color-surface-low, #18181b));
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 12%, transparent);
    border-radius: var(--radius-sm, 4px);
    color: var(--color-foreground, #fff);
    font-family: var(--font-monospace, monospace);
    font-size: 11px;
    outline: none;
    transition: border-color 150ms ease, box-shadow 150ms ease;
  }
  .mono-input:focus {
    border-color: var(--color-accent, #d6202c);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-accent, #d6202c) 25%, transparent);
  }
  .mono-input::placeholder {
    color: color-mix(in srgb, var(--color-foreground, #fff) 25%, transparent);
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
    all: unset;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    cursor: pointer;
    border-radius: 3px;
    color: color-mix(in srgb, var(--color-foreground, #fff) 50%, transparent);
    transition: color 120ms ease, background 120ms ease;
  }
  .input-with-presets :global(.preset-toggle-btn:hover) {
    color: var(--color-foreground, #fff);
    background: color-mix(in srgb, var(--color-foreground, #fff) 10%, transparent);
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
  .clear-path-btn {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: color-mix(in srgb, var(--color-foreground, #fff) 40%, transparent);
    padding: 1px;
    border-radius: 2px;
  }
  .clear-path-btn:hover {
    color: var(--color-foreground, #fff);
    background: color-mix(in srgb, var(--color-foreground, #fff) 15%, transparent);
  }

  .action-pill-btn {
    padding: 0.32rem 0.65rem;
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 14%, transparent);
    border-radius: 999px;
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.08em;
    background: color-mix(in srgb, var(--color-foreground, #fff) 6%, transparent);
    color: var(--color-foreground, #fff);
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease;
    white-space: nowrap;
  }
  .action-pill-btn:hover {
    background: color-mix(in srgb, var(--color-foreground, #fff) 12%, transparent);
    border-color: color-mix(in srgb, var(--color-foreground, #fff) 25%, transparent);
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
    background: color-mix(in srgb, var(--color-foreground, #fff) 2%, var(--color-surface-low, #18181b));
    border-top: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 8%, transparent);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    user-select: none;
  }
  .shortcut-tip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-family: var(--font-text, sans-serif);
    font-size: 10px;
    color: color-mix(in srgb, var(--color-foreground, #fff) 40%, transparent);
  }
  kbd {
    font-family: var(--font-monospace, monospace);
    font-size: 9.5px;
    padding: 0.1rem 0.35rem;
    background: color-mix(in srgb, var(--color-foreground, #fff) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 12%, transparent);
    border-radius: 3px;
    color: color-mix(in srgb, var(--color-foreground, #fff) 75%, transparent);
  }
  .footer-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .btn-cancel {
    padding: 0.38rem 0.9rem;
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 15%, transparent);
    border-radius: 999px;
    font-family: var(--font-header, sans-serif);
    font-size: 11px;
    letter-spacing: 0.08em;
    background: transparent;
    color: color-mix(in srgb, var(--color-foreground, #fff) 80%, transparent);
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease;
  }
  .btn-cancel:hover {
    background: color-mix(in srgb, var(--color-foreground, #fff) 8%, transparent);
    color: var(--color-foreground, #fff);
  }
  .btn-save {
    padding: 0.38rem 1.1rem;
    border: 1px solid var(--color-accent, #d6202c);
    border-radius: 999px;
    font-family: var(--font-header, sans-serif);
    font-size: 11px;
    letter-spacing: 0.08em;
    font-weight: 600;
    background: var(--color-accent, #d6202c);
    color: #fff;
    cursor: pointer;
    box-shadow: 0 2px 8px color-mix(in srgb, var(--color-accent, #d6202c) 35%, transparent);
    transition: filter 120ms ease, transform 100ms ease;
  }
  .btn-save:hover {
    filter: brightness(1.1);
  }
  .btn-save:active {
    transform: scale(0.98);
  }
  .mono {
    font-family: var(--font-monospace, monospace);
  }
</style>
