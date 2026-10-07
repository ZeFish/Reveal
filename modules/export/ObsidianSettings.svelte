<script>
  import { Icon } from "@modules/core";

  /** @typedef {import('@modules/settings/settingsState.svelte.js').Preferences} Preferences */

  /**
   * @typedef {Object} Props
   * @property {Preferences} preferences
   * @property {(key: string) => void} [onChooseFolder]
   */

  /** @type {Props} */
  let {
    preferences = $bindable(),
    onChooseFolder = () => {},
  } = $props();

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
    <Icon name="note-pencil" size="var(--icon-md)" />
    <span>OBSIDIAN INTEGRATION</span>
  </div>
  <div class="card flush list divided">
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
            <code class="path-display" title={preferences.vault || "~/Documents/Atelier (default)"}>
              <Icon name="folder-open" size="var(--icon-md)" />
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
                  <Icon name="x" size="var(--icon-sm)" />
                </button>
              {/if}
            </code>
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
