<script>
  import PhotoGrid from "./PhotoGrid.svelte";

  /**
   * Only the one prop whose shape has already bitten is spelled out; the rest
   * stay loose. `selectGridItem` was once handed the selection module's
   * `selectGridItem(view, index, event)` — three arguments, called here with
   * two — so every mouse click selected nothing, and the untyped default let
   * it through (2026-09-23).
   * @typedef {{ selectGridItem?: (index: number, event?: MouseEvent) => void, [key: string]: any }} Props
   */
  /** @type {Props} */
  let {
    view = [],
    loading = false,
    sel = 0,
    selectedPaths = new Set(),
    storySet = new Set(),
    layout = "uniform",
    cols = 4,
    marginScale = 1,
    cellAspect = 1.0,
    fillCells = true,
    progress = null,
    currentScrollTop = $bindable(0),
    curDir = null,
    minRating = 0,
    filtered = false,
    isTauri = true,
    debug = "",
    selectGridItem = () => {},
    openPhoto = () => {},
    openPhotoMenu = () => {},
    toggleStoryWithPath = () => {},
    onPhotoDragStart = () => {},
    closePhotoMenu = () => {},
    hasRoot = true,
    applePhotosActive = false,
    scanning = false,
    onAddLibraryFolder = () => {},
    gridProseByRow = new Map(),
    onSaveProse = () => {},
    underRail = false,
  } = $props();
</script>

{#if view.length}
  <PhotoGrid
    frames={view}
    {sel}
    {selectedPaths}
    {storySet}
    {layout}
    {cols}
    {marginScale}
    aspect={cellAspect}
    fill={fillCells}
    {progress}
    scrollTop={currentScrollTop}
    onScroll={(/** @type {number} */ val) => {
      currentScrollTop = val;
      closePhotoMenu();
    }}
    onSelect={selectGridItem}
    onDblClick={(/** @type {string} */ path) => openPhoto(path, { openDevPanel: false })}
    onContextMenu={openPhotoMenu}
    onToggleStory={(/** @type {string} */ path) => toggleStoryWithPath(path)}
    onDragStart={onPhotoDragStart}
    {gridProseByRow}
    {onSaveProse}
    {underRail}
  />
{:else if !loading}
  <div class="empty">
    <hgroup>
      <h1 class="text-muted m2">REVEAL</h1>
      {#if applePhotosActive}
        <p>No photos in this view. Try another album or load more photos.</p>
      {:else if !hasRoot}
        <!-- First launch, no catalogue root yet — the one thing a new
             install actually needs before anything else works. A rail
             button already covers this (`indexRoot`, shown when `!root`),
             but it's a small text link off in a thin top bar; a brand new
             user's eyes are on THIS screen, so the real call-to-action
             belongs here too (reproduced 2026-08-03, portability audit). -->
        <p>Add a folder of photos to begin</p>
        <button class="add-library" onclick={() => onAddLibraryFolder()} disabled={!isTauri || scanning}>
          {scanning ? "indexing…" : "Add a folder"}
        </button>
      {:else}
        <p>
          {filtered || (curDir && minRating)
            ? "no photos match this filter"
            : "Looks like an empty place"}
        </p>
      {/if}
      {#if !isTauri}<p><em>Open the Tauri app</em></p>{/if}
      {#if debug}<p class="debug"><em>{debug}</em></p>{/if}
    </hgroup>
  </div>
{/if}

<style>
  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    width: 100%;
    height: 100%;
    min-height: 360px;
  }
  .empty h1 {
    margin: 0 0 calc(var(--space-d4) * 2);
  }
  .empty p {
    opacity: 0.6;
    margin: var(--space-d5) 0;
  }
  .empty p.debug {
    margin-top: calc(var(--space-d4) * 3);
    opacity: 0.4;
  }
  .empty .add-library {
    margin-top: calc(var(--space-d4) * 3);
  }
</style>
