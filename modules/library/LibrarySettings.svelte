<script>
  import { Icon } from "@modules/core";
  import Alert from "@stnd/ui/Alert.svelte";
  import AlertDialog from "@stnd/ui/AlertDialog.svelte";
  import { removeLibraryNote } from "./libraryState.svelte.js";
  import { untrack } from "svelte";

  /**
   * @typedef {Object} Props
   * @property {() => Promise<{path: string, frames: number, online: boolean}[]>} [onListLibraries]
   * @property {() => Promise<void> | void} [onAddLibrary]
   * @property {(path: string) => Promise<void> | void} [onRemoveLibrary]
   * @property {(path: string) => Promise<void> | void} [onRescanLibrary]
   */

  /** @type {Props} */
  let {
    onListLibraries = async () => [],
    onAddLibrary = () => {},
    onRemoveLibrary = () => {},
    onRescanLibrary = () => {},
  } = $props();

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

  $effect(() => {
    untrack(() => {
      loadLibraries();
    });
  });

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
</script>

<div class="section-group">
  <div class="section-heading">
    <Icon name="books" size="12px" />
    <span>CATALOGUED LIBRARIES</span>
  </div>
  <div class="card flush list divided">
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">ADD A LIBRARY</span>
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

  {#if libError}<div role="alert"><Alert class="error">{libError}</Alert></div>{/if}
</div>

<AlertDialog
  open={!!libToRemove}
  title={libToRemove ? `Remove “${libToRemove.path.split("/").pop()}” from Reveal?` : ""}
  description={libToRemove
    ? removeLibraryNote(libToRemove.path, (libraries ?? []).map((l) => l.path), libToRemove.frames)
    : ""}
  confirmLabel="Remove library" cancelLabel="Keep it" intent="danger"
  onconfirm={confirmRemoveLibrary} oncancel={() => { libToRemove = null; }} />
