<script>
  import PhotoGrid from "./PhotoGrid.svelte";

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
    isTauri = true,
    debug = "",
    thumbUrl = (p) => p,
    selectGridItem = () => {},
    openPhoto = () => {},
    openPhotoMenu = () => {},
    toggleStoryWithPath = () => {},
    onPhotoDragStart = () => {},
    closePhotoMenu = () => {},
    hasRoot = true,
    scanning = false,
    onAddLibraryFolder = () => {},
  } = $props();
</script>

{#if view.length}
  <PhotoGrid
    frames={view}
    {sel}
    {selectedPaths}
    {storySet}
    {thumbUrl}
    {layout}
    {cols}
    {marginScale}
    aspect={cellAspect}
    fill={fillCells}
    {progress}
    scrollTop={currentScrollTop}
    onScroll={(val) => {
      currentScrollTop = val;
      closePhotoMenu();
    }}
    onSelect={selectGridItem}
    onDblClick={(path) => openPhoto(path, { openDevPanel: false })}
    onContextMenu={openPhotoMenu}
    onToggleStory={(path) => toggleStoryWithPath(path)}
    onDragStart={onPhotoDragStart}
  />
{:else if !loading}
  <div class="empty">
    <hgroup>
      <h1>REVEAL</h1>
      {#if !hasRoot}
        <!-- First launch, no catalogue root yet — the one thing a new
             install actually needs before anything else works. A rail
             button already covers this (`indexRoot`, shown when `!root`),
             but it's a small text link off in a thin top bar; a brand new
             user's eyes are on THIS screen, so the real call-to-action
             belongs here too (reproduced 2026-08-03, portability audit). -->
        <p>ajoute un dossier de photos pour commencer</p>
        <button class="add-library" onclick={() => onAddLibraryFolder()} disabled={!isTauri || scanning}>
          {scanning ? "indexation…" : "Ajouter un dossier"}
        </button>
      {:else}
        <p>
          {curDir && minRating
            ? "aucune photo à ce filtre"
            : "indexe ta bibliothèque ou ouvre un dossier"}
        </p>
      {/if}
      {#if !isTauri}<p><em>ouvre l'app Tauri</em></p>{/if}
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
    font-family: var(--font-header, sans-serif);
    font-size: 2.4rem;
    letter-spacing: 0.24em;
    color: var(--color-accent);
    margin: 0 0 0.6rem;
  }
  .empty p {
    font-family: var(--font-monospace, monospace);
    font-size: 0.75rem;
    opacity: 0.6;
    margin: 0.2rem 0;
  }
  .empty p.debug {
    margin-top: 0.8rem;
    opacity: 0.4;
  }
  .empty .add-library {
    margin-top: 0.8rem;
  }
</style>
