<script>
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri, Icon, Destination } from "@modules/core";
  import CollapsibleGroup from "@modules/develop/engines/controls/CollapsibleGroup.svelte";

  let {
    installedEditors = [],
    exportFolder,
    exportEdge = $bindable(2048),
    exportBorder = $bindable(false),
    photoPath,
    publishing = $bindable(false),
    publishStatus = $bindable(""),
    activeDestinationId = $bindable("folder"),
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

<div class="export-tab">
  <div class="export-scroll">
    <CollapsibleGroup label="Destination">
      <div class="rows">
        <div class="frow">
          <span class="din frow-label">Send to</span>
          <span class="spacer"></span>
          <select class="panel-select" bind:value={activeDestinationId} aria-label="Destination">
            <option value="folder">Local Folder</option>
            <option value="obsidian">Obsidian Daily Note</option>
            <option value="garden">Garden (Web)</option>
          </select>
        </div>
        {#if installedEditors.length > 0}
          <div class="frow">
            <span class="din frow-label">Editor</span>
            <span class="spacer"></span>
            <select
              class="panel-select"
              aria-label="Open in an editor"
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
      </div>
    </CollapsibleGroup>

    {#if activeDestinationId === "folder" || activeDestinationId === "obsidian"}
      <CollapsibleGroup label="Output">
        <div class="rows">
          {#if activeDestinationId === "folder"}
            <div class="frow">
              <span class="din frow-label">Folder</span>
              <span class="spacer"></span>
              <button class="ghost folder-pick" onclick={() => onChooseExportFolder()} title="Choose the export folder">
                <span class="mono">{exportFolder ? exportFolder.split("/").pop() : "Desktop"}</span>
                <Icon name="folder-open" size="var(--icon-md)" />
              </button>
            </div>
          {/if}
          <div class="frow">
            <span class="din frow-label">Size</span>
            <span class="spacer"></span>
            <select class="panel-select" aria-label="Size" bind:value={exportEdge} onchange={exportSettingsChanged}>
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
        </div>
      </CollapsibleGroup>
    {:else if activeDestinationId === "garden"}
      <CollapsibleGroup label="Publish">
        <div class="rows">
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
      </CollapsibleGroup>
    {/if}
  </div>

  <!-- The action stays in reach, pinned to the bottom like Reset / Export on the Dev tab. -->
  <div class="footer">
    {#if activeDestinationId === "folder"}
      <button class="accent panel-btn" onclick={() => onExport()} disabled={!photoPath}>Export to Folder</button>
    {:else if activeDestinationId === "obsidian"}
      <button class="accent panel-btn" onclick={() => onExportDaily()} disabled={!photoPath}>Send to Daily Note</button>
    {:else if activeDestinationId === "garden"}
      <button class="accent panel-btn" onclick={publishPhoto} disabled={!photoPath || publishing}>
        {publishing ? "Publishing…" : "Publish to Garden"}
      </button>
    {/if}
  </div>
</div>

<style>
  .export-tab {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-d2);
  }

  .export-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    /* No scrollbar: it sat on top of the values (same as the other tabs). */
    scrollbar-width: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }
  .export-scroll > :global(*) {
    /* Groups keep their height and the area scrolls; left to shrink they squeeze each other flat. */
    flex-shrink: 0;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
    padding-block: var(--space-d3);
  }

  .frow {
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    min-height: var(--control-h);
  }
  .frow-label {
    width: 88px;
    flex-shrink: 0;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }

  /* Sizing only — select/button/switch identity (chevron, borders, hover, checked-state fill)
     comes from Standard's own zero-class rules plus the app-wide pill shape in +layout.svelte. */
  .panel-select {
    width: auto;
    max-width: 170px;
    min-height: var(--control-h);
    padding-block: 0;
    padding-inline: var(--space-d2) calc(var(--space-d4) * 6);
  }

  .folder-pick {
    display: inline-flex;
    align-items: center;
    min-height: var(--control-h);
    padding-block: 0;
    padding-inline: var(--space-d2);
    gap: var(--space-d4);
  }
  .folder-pick :global(.icon) {
    color: var(--color-muted);
  }

  .footer {
    flex-shrink: 0;
    padding: var(--space-d3) 0;
    border-top: var(--border);
  }
  .panel-btn {
    display: block;
    width: 100%;
    min-height: var(--control-h);
    padding-block: 0;
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
