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

  // Whether the published note lets a Garden visitor download the full-res
  // JPEG — a per-publish choice (some photos are meant to be looked at, not
  // taken), so it lives here as a plain toggle rather than a persisted
  // develop setting like exportEdge/exportBorder above.
  const ALLOW_DOWNLOAD_KEY = "reveal.publish.allowDownload";
  let allowDownload = $state(
    typeof localStorage !== "undefined" && localStorage.getItem(ALLOW_DOWNLOAD_KEY) === "true"
  );
  function toggleAllowDownload() {
    allowDownload = !allowDownload;
    try { localStorage.setItem(ALLOW_DOWNLOAD_KEY, String(allowDownload)); } catch (_) {}
  }

  async function publishPhoto() {
    if (!photoPath || publishing || !isTauri) return;
    publishing = true;
    publishStatus = "";
    try {
      const live = await invoke("publish_photo", { path: photoPath, allowDownload });
      publishStatus = live;
      // Hand the finished page straight back: clipboard has the link ready
      // to paste, and the browser tab is already open on it.
      try { await navigator.clipboard.writeText(live); } catch (_) {}
      invoke("open_path", { path: live }).catch(() => {});
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
      <span class="din frow-label">Size</span>
      <span class="spacer"></span>
      <select class="panel-select" bind:value={exportEdge} onchange={exportSettingsChanged}>
        <option value={0}>Full</option>
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
    <button class="outline panel-btn" onclick={() => onExportDaily()} disabled={!photoPath}>Daily note (Obsidian)</button>
    <div class="frow">
      <span class="din frow-label">Allow download</span>
      <span class="spacer"></span>
      <input
        type="checkbox"
        role="switch"
        aria-label="Allow download"
        checked={allowDownload}
        onchange={toggleAllowDownload}
      />
    </div>
    <button class="secondary panel-btn" onclick={publishPhoto} disabled={!photoPath || publishing}>
      {publishing ? "Publishing…" : "Publish (Garden)"}
    </button>
    {#if publishStatus}
      {#if publishStatus.startsWith("http")}
        <a
          class="hint published-link"
          href={publishStatus}
          onclick={(e) => { e.preventDefault(); invoke("open_path", { path: publishStatus }); }}
          title="Open the published page"
        >{publishStatus}</a>
      {:else}
        <p class="hint">{publishStatus}</p>
      {/if}
    {/if}
  </div>
</div>

<style>
  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space);
  }

  .sec-body {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 3);
  }

  .frow {
    display: flex;
    align-items: center;
    gap: var(--space-d2);
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
    padding: var(--space-d8) calc(var(--space-d4) * 6) var(--space-d8) var(--space-d2);
  }

  .folder-pick {
    padding: var(--space-d8) var(--space-d2);
    gap: var(--space-d4);
  }
  .folder-pick :global(.icon) {
    color: var(--color-muted);
  }

  .panel-btn {
    display: block;
    width: 100%;
    padding: var(--space-d2) 0;
    margin-top: var(--space-d4);
  }

  .hint {
    margin: 0;
  }
  .published-link {
    display: block;
    word-break: break-all;
    cursor: pointer;
  }
</style>
