<script>
  import { Icon } from "@modules/core";
  import Dropdown from "@stnd/ui/Dropdown.svelte";
  import DropdownItem from "@stnd/ui/DropdownItem.svelte";

  /** @typedef {import('@modules/settings/settingsState.svelte.js').Preferences} Preferences */

  /**
   * @typedef {Object} Props
   * @property {Preferences} preferences
   * @property {boolean} [autoImportEnabled]
   * @property {(enabled: boolean) => Promise<void> | void} [onToggleAutoImport]
   * @property {{name: string, recipe: any}[]} [presets]
   * @property {string | null} [defaultImportPreset]
   * @property {(name: string | null) => Promise<void> | void} [onSetDefaultImportPreset]
   * @property {(key: string) => void} [onChooseFolder]
   */

  /** @type {Props} */
  let {
    preferences = $bindable(),
    autoImportEnabled = false,
    onToggleAutoImport = () => {},
    presets = [],
    defaultImportPreset = null,
    onSetDefaultImportPreset = () => {},
    onChooseFolder = () => {},
  } = $props();

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

<div class="section-group">
  <div class="section-heading">
    <Icon name="arrows-down-up" size="var(--icon-md)" />
    <span>PHOTO ORGANIZATION &amp; IMPORT</span>
  </div>
  <div class="card flush list divided date-card">
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
            {#snippet trigger()}
              <Icon name="caret-down" size="var(--icon-sm)" />
            {/snippet}
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
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">DEFAULT IMPORT PRESET</span>
        <span class="row-desc">Applied automatically to every photo that arrives from a memory card.</span>
      </div>
      <div class="row-control">
        <Dropdown label="Default import preset" triggerClass="outline small action-pill-btn" align="end">
          {#snippet trigger()}
            <span>{defaultImportPreset ?? "None"}</span>
            <Icon name="caret-down" size="var(--icon-sm)" />
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

<div class="section-group">
  <div class="section-heading">
    <Icon name="folder-open" size="var(--icon-md)" />
    <span>EXPORT &amp; ASSETS</span>
  </div>
  <div class="card flush list divided">
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">DEFAULT EXPORT FOLDER</span>
        <span class="row-desc">Destination for developed JPEGs outside the vault. Defaults to the Desktop.</span>
      </div>
      <div class="row-control">
        <div class="path-picker-group">
          <code class="path-display" title={preferences.export_folder || "Desktop (default)"}>
            <Icon name="folder-open" size="var(--icon-md)" />
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
                <Icon name="x" size="var(--icon-sm)" />
              </button>
            {/if}
          </code>
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
          <code class="path-display" title={preferences.lut_folder || "Built-in LUTs only"}>
            <Icon name="folder-open" size="var(--icon-md)" />
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
                <Icon name="x" size="var(--icon-sm)" />
              </button>
            {/if}
          </code>
          <button type="button" class="outline small action-pill-btn" onclick={() => onChooseFolder("lut_folder")}>
            CHOOSE…
          </button>
        </div>
      </div>
    </div>
  </div>
</div>
