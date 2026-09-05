<script>
  import Icon from "$lib/components/Icon.svelte";

  /**
   * @typedef {Object} Preferences
   * @property {string} date_folders
   * @property {string} vault
   * @property {string} logs_folder
   * @property {string} export_folder
   * @property {string} lut_folder
   * @property {boolean} ai_cull_enabled
   * @property {number} ai_cull_target
   * @property {string} ai_api_key
   */
  /**
   * @typedef {Object} Props
   * @property {Preferences} [preferences]
   * @property {() => void} [onClose]
   * @property {(key: string) => void} [onChooseFolder]
   * @property {() => void} [onSave]
   */

  /** @type {Props} */
  let {
    preferences = $bindable({
      date_folders: "%Y/%Y-%m-%d",
      vault: "",
      logs_folder: "Logs",
      export_folder: "",
      lut_folder: "",
      ai_cull_enabled: false,
      ai_cull_target: 24,
      ai_api_key: "",
    }),
    onClose = () => {},
    onChooseFolder = (/** @type {string} */ key) => {},
    onSave = () => {},
  } = $props();

  let showDateExamples = $state(false);

  const datePresets = [
    { pattern: "%Y/%Y-%m-%d", example: "2026/2026-12-31", desc: "Année / Année-Mois-Jour" },
    { pattern: "%Y/%m/%d", example: "2026/12/31", desc: "Année / Mois / Jour" },
    { pattern: "%Y-%m-%d", example: "2026-12-31", desc: "Année-Mois-Jour à plat" },
    { pattern: "%Y/%m", example: "2026/12", desc: "Année / Mois" },
  ];

  /** @param {string} p */
  function selectDatePattern(p) {
    preferences.date_folders = p;
    showDateExamples = false;
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
      onSave();
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

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="settings-overlay" onclick={onClose} onkeydown={handleKeyDown} role="presentation">
  <div
    class="settings-panel"
    onclick={(event) => event.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-label="Réglages Reveal"
    tabindex="-1"
  >
    <!-- Modal Header -->
    <header class="settings-header">
      <div class="header-left">
        <span class="settings-badge">
          <Icon name="gear" size="14px" />
        </span>
        <h2>RÉGLAGES REVEAL</h2>
      </div>
      <button class="close-btn" onclick={onClose} aria-label="Fermer les réglages" title="Fermer (Échap)">
        <Icon name="x" size="14px" />
      </button>
    </header>

    <!-- Modal Scrollable Content -->
    <div class="settings-scroll">
      <!-- Section: Organisation Photos -->
      <div class="section-group">
        <div class="section-heading">
          <Icon name="image" size="12px" />
          <span>ORGANISATION DES PHOTOS</span>
        </div>

        <div class="inset-card">
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
                <button
                  type="button"
                  class="preset-toggle-btn"
                  onclick={() => (showDateExamples = !showDateExamples)}
                  title="Modèles prédéfinis"
                >
                  <Icon name="caret-down" size="10px" />
                </button>
              </div>

              {#if showDateExamples}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <div class="presets-dropdown" onclick={(e) => e.stopPropagation()} role="menu">
                  {#each datePresets as preset}
                    <button
                      type="button"
                      class="preset-item"
                      class:active={preferences.date_folders === preset.pattern}
                      onclick={() => selectDatePattern(preset.pattern)}
                      role="menuitem"
                    >
                      <span class="preset-code">{preset.pattern}</span>
                      <span class="preset-desc">{preset.desc}</span>
                      <span class="preset-example">{preset.example}</span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        </div>
      </div>

      <!-- Section: Obsidian & Chemins -->
      <div class="section-group">
        <div class="section-heading">
          <Icon name="folder-open" size="12px" />
          <span>OBSIDIAN &amp; EMPLACEMENTS</span>
        </div>

        <div class="inset-card">
          <!-- Vault Obsidian -->
          <div class="setting-row">
            <div class="row-meta">
              <span class="row-label">RACINE DU VAULT OBSIDIAN</span>
              <span class="row-desc">Dossier racine pour l'écriture des fiches catalogues et notes journalières.</span>
            </div>
            <div class="row-control">
              <div class="path-picker-group">
                <div class="path-display" title={preferences.vault || "Aucun dossier sélectionné"}>
                  <Icon name="folder-open" size="12px" />
                  <span class="path-text mono">
                    {preferences.vault ? formatPath(preferences.vault) : "Non configuré (optionnel)"}
                  </span>
                  {#if preferences.vault}
                    <button
                      type="button"
                      class="clear-path-btn"
                      onclick={() => (preferences.vault = "")}
                      title="Effacer"
                    >
                      <Icon name="x" size="10px" />
                    </button>
                  {/if}
                </div>
                <button
                  type="button"
                  class="action-pill-btn"
                  onclick={() => onChooseFolder("vault")}
                >
                  CHOISIR…
                </button>
              </div>
            </div>
          </div>

          <!-- Notes quotidiennes -->
          <div class="setting-row">
            <div class="row-meta">
              <span class="row-label">SOUS-DOSSIER DES NOTES QUOTIDIENNES</span>
              <span class="row-desc">Emplacement relatif dans le vault (ex : Logs, Journal, Logs/Daily).</span>
            </div>
            <div class="row-control">
              <input
                class="mono-input"
                bind:value={preferences.logs_folder}
                placeholder="Logs"
                spellcheck="false"
              />
            </div>
          </div>

          <!-- Dossier d'export -->
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
                <button
                  type="button"
                  class="action-pill-btn"
                  onclick={() => onChooseFolder("export_folder")}
                >
                  CHOISIR…
                </button>
              </div>
            </div>
          </div>

          <!-- Dossier LUT -->
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
                <button
                  type="button"
                  class="action-pill-btn"
                  onclick={() => onChooseFolder("lut_folder")}
                >
                  CHOISIR…
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Section: Intelligence Artificielle & Culling -->
      <div class="section-group">
        <div class="section-heading">
          <Icon name="lightning" size="12px" />
          <span>AI CULLING &amp; AUTOMATISATION</span>
        </div>

        <div class="inset-card">
          <!-- Toggle Auto-Cull -->
          <div class="setting-row">
            <div class="row-meta">
              <span class="row-label">CULL &amp; EXPORT AUTOMATIQUES</span>
              <span class="row-desc">Évalue par vision IA et exporte la sélection automatiquement après chaque import.</span>
            </div>
            <div class="row-control">
              <input type="checkbox" role="switch" bind:checked={preferences.ai_cull_enabled} />
            </div>
          </div>

          <!-- Cible de conservation -->
          <div class="setting-row" class:row-disabled={!preferences.ai_cull_enabled}>
            <div class="row-meta">
              <span class="row-label">PHOTOS À RETENIR PAR SESSION</span>
              <span class="row-desc">Nombre cible de photos retenues par dossier de date lors du tri automatique.</span>
            </div>
            <div class="row-control">
              <div class="stepper-group">
                <button
                  type="button"
                  class="stepper-btn"
                  disabled={!preferences.ai_cull_enabled || preferences.ai_cull_target <= 1}
                  onclick={() => adjustTarget(-1)}
                  title="Diminuer"
                >−</button>
                <input
                  type="number"
                  class="stepper-input mono"
                  min="1"
                  max="200"
                  disabled={!preferences.ai_cull_enabled}
                  bind:value={preferences.ai_cull_target}
                />
                <button
                  type="button"
                  class="stepper-btn"
                  disabled={!preferences.ai_cull_enabled || preferences.ai_cull_target >= 200}
                  onclick={() => adjustTarget(1)}
                  title="Augmenter"
                >+</button>
              </div>
            </div>
          </div>

          <!-- Clé API Vision -->
          <div class="setting-row" class:row-disabled={!preferences.ai_cull_enabled}>
            <div class="row-meta">
              <span class="row-label">CLÉ API VISION (ANTHROPIC)</span>
              <span class="row-desc">Envoie des vignettes compressées pour classement. Facturé selon l'usage d'API.</span>
            </div>
            <div class="row-control">
              <input
                type="password"
                class="mono-input"
                disabled={!preferences.ai_cull_enabled}
                bind:value={preferences.ai_api_key}
                placeholder="sk-ant-…"
                autocomplete="off"
                spellcheck="false"
              />
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Modal Footer -->
    <footer class="settings-footer">
      <div class="footer-shortcuts">
        <span class="shortcut-tip"><kbd>Échap</kbd> Annuler</span>
        <span class="shortcut-tip"><kbd>⌘</kbd><kbd>Entrée</kbd> Enregistrer</span>
      </div>
      <div class="footer-actions">
        <button type="button" class="btn-cancel" onclick={onClose}>
          ANNULER
        </button>
        <button type="button" class="btn-save" onclick={onSave}>
          ENREGISTRER
        </button>
      </div>
    </footer>
  </div>
</div>

<style>
  /* Base overlay with backdrop blur */
  .settings-overlay {
    position: fixed;
    inset: 0;
    z-index: 10000;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
    animation: fadeIn 150ms ease-out;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes panelIn {
    from {
      opacity: 0;
      transform: scale(0.97) translateY(6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  /* Main Dialog Panel */
  .settings-panel {
    box-sizing: border-box;
    background: var(--color-surface-low, #18181b);
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 12%, transparent);
    border-radius: var(--radius-lg, 12px);
    width: min(94vw, 580px);
    max-height: min(88vh, 720px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow:
      0 24px 48px -12px rgba(0, 0, 0, 0.7),
      0 0 0 1px rgba(255, 255, 255, 0.05);
    color: var(--color-foreground, #f4f4f5);
    animation: panelIn 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  /* Header */
  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    background: color-mix(in srgb, var(--color-foreground, #fff) 2%, var(--color-surface-low, #18181b));
    border-bottom: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 8%, transparent);
    user-select: none;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .settings-badge {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm, 4px);
    background: color-mix(in srgb, var(--color-accent, #d6202c) 15%, transparent);
    color: var(--color-accent, #d6202c);
  }

  .settings-header h2 {
    font-family: var(--font-header, sans-serif);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.1em;
    margin: 0;
    color: var(--color-foreground, #f4f4f5);
  }

  .close-btn {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    color: color-mix(in srgb, var(--color-foreground, #fff) 60%, transparent);
    transition: background 120ms ease, color 120ms ease;
  }

  .close-btn:hover {
    background: color-mix(in srgb, var(--color-foreground, #fff) 10%, transparent);
    color: var(--color-foreground, #fff);
  }

  /* Scrollable Body */
  .settings-scroll {
    padding: 1.25rem;
    overflow-y: auto;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  /* Section Groups & Headings */
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

  /* Inset Grouped Cards */
  .inset-card {
    background: color-mix(in srgb, var(--color-foreground, #fff) 3.5%, var(--color-surface-low, #18181b));
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 8%, transparent);
    border-radius: var(--radius, 8px);
    overflow: hidden;
  }

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

  /* Text & Monospace Inputs */
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

  /* Date Control with Presets Dropdown */
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

  .preset-toggle-btn {
    all: unset;
    position: absolute;
    right: 4px;
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

  .preset-toggle-btn:hover {
    color: var(--color-foreground, #fff);
    background: color-mix(in srgb, var(--color-foreground, #fff) 10%, transparent);
  }

  .presets-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 200;
    width: 280px;
    background: var(--color-surface-high, #242427);
    border: 1px solid color-mix(in srgb, var(--color-foreground, #fff) 15%, transparent);
    border-radius: var(--radius, 6px);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.5);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .preset-item {
    all: unset;
    display: flex;
    flex-direction: column;
    padding: 6px 8px;
    border-radius: 4px;
    cursor: pointer;
    gap: 2px;
    transition: background 100ms ease;
  }

  .preset-item:hover,
  .preset-item.active {
    background: color-mix(in srgb, var(--color-foreground, #fff) 10%, transparent);
  }

  .preset-item.active {
    border-left: 2px solid var(--color-accent, #d6202c);
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

  /* Path Picker Group */
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

  /* Custom Stepper */
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
  }

  .stepper-input::-webkit-outer-spin-button,
  .stepper-input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  /* Footer */
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

  .footer-shortcuts {
    display: flex;
    align-items: center;
    gap: 0.75rem;
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
