<script>
  import { emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";

  let {
    installedEditors = [],
    exportFolder,
    exportEdge = $bindable(2048),
    exportBorder = $bindable(false),
    photoPath,
    publishing = $bindable(false),
    publishStatus = $bindable(""),
  } = $props();

  function exportSettingsChanged() {
    emit("dev-panel-export-settings-changed", { exportEdge, exportBorder });
  }

  function triggerExport() {
    emit("dev-panel-export", {});
  }

  function chooseExportFolder() {
    emit("dev-panel-choose-export-folder", {});
  }

  /** @param {string} appPath */
  function openInEditor(appPath) {
    if (appPath) emit("dev-panel-open-in-editor", { appPath });
  }

  async function publishPhoto() {
    if (!photoPath || publishing || !isTauri) return;
    publishing = true;
    publishStatus = "";
    try {
      const live = await invoke("publish_photo", { path: photoPath });
      publishStatus = live;
    } catch (e) {
      publishStatus = `Error: ${e}`;
    } finally {
      publishing = false;
    }
  }
</script>

<div class="pane-scroll">
  <div class="sec-body">
    {#if installedEditors.length > 0}
      <div class="frow">
        <span class="din frow-label">Éditeur</span>
        <span class="spacer"></span>
        <span class="pick">
          <select
            onchange={(e) => openInEditor(e.currentTarget.value)}
            value=""
          >
            <option value="" disabled selected>Ouvrir dans…</option>
            {#each installedEditors as [name, path]}
              <option value={path}>{name}</option>
            {/each}
          </select>
          <Icon name="caret-down" size="8px" />
        </span>
      </div>
    {/if}
    <div class="frow">
      <span class="din frow-label">Dossier</span>
      <span class="spacer"></span>
      <button
        class="folder-pick"
        onclick={chooseExportFolder}
        title="Choisir le dossier d'export"
      >
        <span class="mono">{exportFolder ? exportFolder.split("/").pop() : "Bureau"}</span>
        <Icon name="folder-open" size="10px" />
      </button>
    </div>
    <div class="frow">
      <span class="din frow-label">Taille</span>
      <span class="spacer"></span>
      <span class="pick">
        <select bind:value={exportEdge} onchange={exportSettingsChanged}>
          <option value={0}>Plein</option>
          <option value={4096}>4096</option>
          <option value={2048}>2048</option>
          <option value={1600}>1600</option>
          <option value={1024}>1024</option>
        </select>
        <Icon name="caret-down" size="8px" />
      </span>
    </div>
    <div class="frow">
      <span class="din frow-label">White border</span>
      <span class="spacer"></span>
      <button
        class="toggle"
        class:on={exportBorder}
        role="switch"
        aria-label="White border"
        aria-checked={exportBorder}
        onclick={() => {
          exportBorder = !exportBorder;
          exportSettingsChanged();
        }}
      ><span class="knob"></span></button>
    </div>
    <button class="capsule fill" onclick={triggerExport} disabled={!photoPath}>Export</button>
    <button class="capsule accent" onclick={publishPhoto} disabled={!photoPath || publishing}>
      {publishing ? "Publishing…" : "Publish"}
    </button>
    {#if publishStatus}
      <p class="hint">{publishStatus}</p>
    {/if}
  </div>
</div>

<style>
  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .sec-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .din {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }

  .mono {
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    color: color-mix(in srgb, var(--color-foreground) 90%, transparent);
  }

  .frow {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 16px;
  }
  .frow-label {
    width: 88px;
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }

  .pick {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 2px 8px;
    max-width: 170px;
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
  }
  .pick select {
    all: unset;
    -webkit-appearance: none;
    appearance: none;
    display: block;
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    color: inherit;
    width: 100%;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }
  .pick :global(.icon) {
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
    pointer-events: none;
    flex-shrink: 0;
  }

  .folder-pick {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
  }
  .folder-pick:hover {
    color: var(--color-foreground);
  }
  .folder-pick :global(.icon) {
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }

  .toggle {
    all: unset;
    cursor: pointer;
    width: 28px;
    height: 16px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--color-foreground) 15%, transparent);
    position: relative;
    transition: background var(--duration-fast) var(--ease-soft);
    flex-shrink: 0;
  }
  .toggle.on {
    background: var(--color-accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-surface-high);
    transition: transform var(--duration-fast) var(--ease-soft);
  }
  .toggle.on .knob {
    transform: translateX(12px);
  }

  .capsule {
    all: unset;
    display: block;
    width: 100%;
    box-sizing: border-box;
    cursor: pointer;
    text-align: center;
    padding: 8px 0;
    border-radius: 999px;
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    margin-top: 4px;
  }
  .capsule.fill {
    color: var(--color-background);
    background: var(--color-foreground);
  }
  .capsule.accent {
    color: #fff;
    background: var(--color-accent);
  }
  .capsule:disabled {
    opacity: 0.25;
    cursor: default;
  }

  .hint {
    font-family: var(--font-text, sans-serif);
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
    margin: 0;
  }
</style>
