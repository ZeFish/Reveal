<script>
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
    // What each of these actually DOES differs by host (docked: call the
    // main window's own function directly; detached: relay over IPC) — see
    // DevelopPanel.svelte. publishPhoto stays local below since it's a plain
    // Tauri command either way, nothing host-specific about it.
    onExportSettingsChanged = () => {},
    onExport = () => {},
    onExportDaily = () => {},
    onChooseExportFolder = () => {},
    /** @param {string} appPath */
    onOpenInEditor = () => {},
  } = $props();

  function exportSettingsChanged() {
    onExportSettingsChanged({ exportEdge, exportBorder });
  }

  /** @param {string} appPath */
  function openInEditor(appPath) {
    if (appPath) onOpenInEditor(appPath);
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
        <span class="din frow-label">Editor</span>
        <span class="spacer"></span>
        <select
          class="panel-select"
          onchange={(e) => openInEditor(e.currentTarget.value)}
          value=""
        >
          <option value="" disabled selected>Open in…</option>
          {#each installedEditors as [name, path]}
            <option value={path}>{name}</option>
          {/each}
        </select>
      </div>
    {/if}
    <div class="frow">
      <span class="din frow-label">Folder</span>
      <span class="spacer"></span>
      <button
        class="ghost folder-pick"
        onclick={() => onChooseExportFolder()}
        title="Choose the export folder"
      >
        <span class="mono">{exportFolder ? exportFolder.split("/").pop() : "Desktop"}</span>
        <Icon name="folder-open" size="10px" />
      </button>
    </div>
    <div class="frow">
      <span class="din frow-label">Taille</span>
      <span class="spacer"></span>
      <select class="panel-select" bind:value={exportEdge} onchange={exportSettingsChanged}>
        <option value={0}>Plein</option>
        <option value={4096}>4096</option>
        <option value={2048}>2048</option>
        <option value={1600}>1600</option>
        <option value={1024}>1024</option>
      </select>
    </div>
    <div class="frow">
      <span class="din frow-label">White border</span>
      <span class="spacer"></span>
      <input
        type="checkbox"
        role="switch"
        aria-label="White border"
        checked={exportBorder}
        onchange={() => {
          exportBorder = !exportBorder;
          exportSettingsChanged();
        }}
      />
    </div>
    <button class="secondary panel-btn" onclick={() => onExport()} disabled={!photoPath}>Export</button>
    <button class="outline panel-btn" onclick={() => onExportDaily()} disabled={!photoPath}>Note du jour (Obsidian)</button>
    <button class="secondary panel-btn" onclick={publishPhoto} disabled={!photoPath || publishing}>
      {publishing ? "Publishing…" : "Publish (Garden)"}
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

  /* Sizing only — select/button/switch identity (chevron, borders, hover,
     checked-state fill) all come from Standard's own zero-class rules
     (packages/styles/_standard-11-forms.scss, _standard-13-components.scss)
     plus the app-wide pill shape in +layout.svelte. This panel just needs
     everything smaller and right-aligned than either provides by default. */
  .panel-select {
    width: auto;
    max-width: 170px;
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    padding: 2px 22px 2px 8px;
  }

  .folder-pick {
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    padding: 2px 8px;
    gap: 4px;
  }
  .folder-pick :global(.icon) {
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }

  .panel-btn {
    display: block;
    width: 100%;
    padding: 8px 0;
    font-size: 10.8px;
    letter-spacing: 0.12em;
    margin-top: 4px;
  }

  .hint {
    font-family: var(--font-text, sans-serif);
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
    margin: 0;
  }
</style>
