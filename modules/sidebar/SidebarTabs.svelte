<script>
  /**
   * @typedef {Object} Props
   * @property {boolean} previewFilter
   * @property {boolean} isLibrary
   * @property {boolean} applePhotosActive
   * @property {() => void} onTogglePreview
   */

  /** @type {Props} */
  let {
    previewFilter = false,
    isLibrary = false,
    applePhotosActive = false,
    onTogglePreview = () => {},
  } = $props();
</script>

<!-- FRAMES ↔ EDITORIAL — a display filter on the same grid, not a mode.
     Both tabs call the same toggle; only Editorial is guarded (nothing to
     compose without a folder or in the read-only Apple Photos library). -->
<div class="tabs btn-group" role="group" aria-label="View">
  <button aria-pressed={!previewFilter} onclick={() => previewFilter && onTogglePreview()}>Frames</button>
  <button
    aria-pressed={previewFilter}
    disabled={isLibrary || applePhotosActive}
    onclick={() => !previewFilter && onTogglePreview()}
    title={isLibrary ? "Choose a folder to compose a story" : "Editorial (S)"}
  >Editorial</button>
</div>

<style>
  .tabs {
    flex-shrink: 0;
  }
</style>
