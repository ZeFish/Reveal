<script>
  let {
    preferences = $bindable({
      date_folders: "",
      vault: "",
      logs_folder: "",
      export_folder: "",
      lut_folder: "",
    }),
    onClose = () => {},
    onChooseFolder = (/** @type {string} */ key) => {},
    onSave = () => {},
  } = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="settings-overlay" onclick={onClose} role="presentation">
  <dialog
    open
    class="settings-panel"
    onclick={(event) => event.stopPropagation()}
    onkeydown={(event) => event.stopPropagation()}
    aria-label="Reveal settings"
  >
    <header class="settings-header">
      <h2>REVEAL SETTINGS</h2>
      <button onclick={onClose} aria-label="Close settings">×</button>
    </header>
    <div class="settings-scroll">
      <fieldset>
        <legend>PHOTOS</legend>
        <label>
          <span>DATE FOLDER FORMAT</span>
          <input bind:value={preferences.date_folders} placeholder="%Y/%Y-%m-%d" />
          <small>Example: 2026/2026-12-31</small>
        </label>
      </fieldset>
      <fieldset>
        <legend>OBSIDIAN</legend>
        <label>
          <span>VAULT ROOT</span>
          <div class="settings-picker">
            <input bind:value={preferences.vault} />
            <button onclick={() => onChooseFolder("vault")}>CHOOSE…</button>
          </div>
        </label>
        <label>
          <span>DAILY NOTES</span>
          <input bind:value={preferences.logs_folder} placeholder="Logs" />
        </label>
        <label>
          <span>EXPORT FOLDER</span>
          <div class="settings-picker">
            <input bind:value={preferences.export_folder} placeholder="Desktop when empty" />
            <button onclick={() => onChooseFolder("export_folder")}>CHOOSE…</button>
          </div>
        </label>
        <label>
          <span>LUT FOLDER</span>
          <div class="settings-picker">
            <input bind:value={preferences.lut_folder} />
            <button onclick={() => onChooseFolder("lut_folder")}>CHOOSE…</button>
          </div>
        </label>
      </fieldset>
    </div>
    <footer class="settings-footer">
      <button onclick={onClose}>CANCEL</button>
      <button class="primary" onclick={onSave}>SAVE</button>
    </footer>
  </dialog>
</div>

<style>
  .settings-overlay {
    position: fixed;
    inset: 0;
    z-index: 10000;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .settings-panel {
    all: unset;
    box-sizing: border-box;
    background: var(--color-surface-low, #18181b);
    border: var(--border, 1px solid rgba(255, 255, 255, 0.15));
    border-radius: var(--radius-lg);
    width: min(90vw, 520px);
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
    color: var(--color-foreground, #f4f4f5);
  }
  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
  }
  .settings-header h2 {
    font-family: var(--font-header, sans-serif);
    font-size: 0.9rem;
    letter-spacing: 0.1em;
    margin: 0;
  }
  .settings-header button {
    all: unset;
    cursor: pointer;
    opacity: 0.6;
    font-size: 1.2rem;
    padding: 0.2rem;
  }
  .settings-header button:hover {
    opacity: 1;
  }
  .settings-scroll {
    padding: 1.25rem;
    overflow-y: auto;
    flex: 1;
  }
  .settings-scroll fieldset {
    display: grid;
    gap: 16px;
    margin: 0 0 28px;
    padding: 0;
    border: 0;
  }
  .settings-scroll legend,
  .settings-scroll label > span {
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.12em;
  }
  .settings-scroll legend {
    width: 100%;
    margin-bottom: 2px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
    color: color-mix(in srgb, var(--color-foreground, #fff) 55%, transparent);
  }
  .settings-scroll label {
    display: grid;
    gap: 6px;
  }
  .settings-scroll input {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    padding: 7px 9px;
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.15));
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--color-muted, #999) 5%, transparent);
    color: var(--color-foreground, #fff);
    font-family: var(--font-monospace, monospace);
    font-size: 11px;
  }
  .settings-scroll small {
    color: color-mix(in srgb, var(--color-foreground, #fff) 50%, transparent);
    font-size: 10px;
  }
  .settings-picker {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
  }
  .settings-picker button,
  .settings-footer button {
    padding: 6px 10px;
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.15));
    border-radius: 999px;
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.1em;
    background: var(--color-surface-high, #27272a);
    color: var(--color-foreground, #fff);
    cursor: pointer;
  }
  .settings-footer {
    padding: 0.75rem 1.25rem;
    border-top: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .settings-footer .primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: #fff;
  }
</style>
