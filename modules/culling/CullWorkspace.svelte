<script>
  import { invoke } from "@tauri-apps/api/core";
  import { Sidebar } from "@modules/sidebar";
  import CullTopRail from "./CullTopRail.svelte";
  import CullView from "./CullView.svelte";
  import { StoryView } from "@modules/story";
  import { ContextMenu } from "@modules/menus";
  import { Photo, thumbUrl, virtualCollections } from "@modules/core";

  /**
   * @typedef {Object} Props
   * @property {any[]} view
   * @property {number} sel
   * @property {any} selection
   * @property {any} library
   * @property {any} libraryController
   * @property {any} navigationCtrl
   * @property {any} sidebarCtrl
   * @property {any} modeCtrl
   * @property {any} cullingCtrl
   * @property {any} cullingState
   * @property {any} storyCtrl
   * @property {any} storyState
   * @property {any} importCtrl
   * @property {any} importState
   * @property {any} photoMenuCtrl
   * @property {any} modalState
   * @property {any} tidyState
   * @property {any} settingsCtrl
   * @property {any} exportCtrl
   * @property {any} applePhotos
   * @property {any} immich
   * @property {any} applePhotosLibrary
   * @property {any} immichLibrary
   * @property {any} gardenAccount
   * @property {any} preferences
   * @property {any} activity
   * @property {string} status
   * @property {string} debug
   * @property {number} currentScrollTop
   * @property {string} currentMode
   * @property {any} layouts
   * @property {boolean} isTauri
   * @property {boolean} masonryTooBig
   * @property {number} masonryLimit
   * @property {(path: string) => string | undefined} dirLabel
   * @property {any[]} installedEditors
   * @property {any} developState
   * @property {any} gridProseByRow
   * @property {(e: MouseEvent) => void} onStartWindowDrag
   * @property {() => void} toggleAppearance
   * @property {(path: string) => Promise<any>} copyImageToClipboard
   * @property {() => void} copySettings
   * @property {() => void} pasteSettings
   * @property {(view: any[], i: number, e?: MouseEvent) => void} selectGridItem
   * @property {(desc: boolean) => Promise<any>} loadMoreApplePhotos
   * @property {(desc: boolean) => Promise<any>} loadMoreImmich
   * @property {() => Promise<any>} connectApplePhotos
   * @property {(onActive?: (album: string) => any) => Promise<any>} refreshApplePhotos
   * @property {(album: string) => any} handleRefreshActive
   * @property {(prefs: any, refresh?: boolean) => Promise<any>} loadImmichCollections
   * @property {(path: string, opts?: any) => Promise<any>} openPhoto
   * @property {(path: string) => Promise<any>} openPhotoPreview
   * @property {(path: string) => Promise<any>} revealPhotoInFinder
   * @property {(path: string, app: string) => Promise<any>} openPhotoInEditor
   */

  /** @type {Props} */
  let {
    view,
    sel,
    selection,
    library,
    libraryController,
    navigationCtrl,
    sidebarCtrl,
    modeCtrl,
    cullingCtrl,
    cullingState,
    storyCtrl,
    storyState,
    importCtrl,
    importState,
    photoMenuCtrl,
    modalState,
    tidyState,
    settingsCtrl,
    exportCtrl,
    applePhotos,
    immich,
    applePhotosLibrary,
    immichLibrary,
    gardenAccount,
    preferences,
    activity,
    status,
    debug,
    currentScrollTop = $bindable(0),
    currentMode,
    layouts,
    isTauri,
    masonryTooBig,
    masonryLimit,
    dirLabel,
    installedEditors,
    developState,
    gridProseByRow,
    onStartWindowDrag,
    toggleAppearance,
    copyImageToClipboard,
    copySettings,
    pasteSettings,
    selectGridItem,
    loadMoreApplePhotos,
    loadMoreImmich,
    connectApplePhotos,
    refreshApplePhotos,
    handleRefreshActive,
    loadImmichCollections,
    openPhoto,
    openPhotoPreview,
    revealPhotoInFinder,
    openPhotoInEditor,
  } = $props();

  const sidebarPeek = $derived(sidebarCtrl.peek);
  const sidebarVisible = $derived(sidebarCtrl.isVisible);
  const previewFilter = $derived(modeCtrl.previewFilter);

  /** @param {string} name */
  const stem = (name) => name.replace(/\.[^.]+$/, "");
</script>

<div
  class="cull"
  role="presentation"
>
  {#snippet sidebarPanel(/** @type {boolean} */ floating, /** @type {() => void} */ afterNavigate)}
    <Sidebar
      {floating}
      onOpenDir={(/** @type {string} */ path) => {
        navigationCtrl.openDir(path);
        afterNavigate();
      }}
      onOpenLibrary={() => {
        navigationCtrl.openDir(library.root);
        afterNavigate();
      }}
      applePhotos={applePhotosLibrary}
      onConnectApplePhotos={connectApplePhotos}
      onRefreshApplePhotos={() => refreshApplePhotos(handleRefreshActive)}
      immich={immichLibrary}
      onConnectImmich={() => loadImmichCollections(preferences)}
      onRefreshImmich={() => loadImmichCollections(preferences, true)}
      root={library.root}
      roots={library.roots}
      dirs={library.dirs}
      curDir={library.curDir}
      collections={virtualCollections.withCounts(library.frames, storyState.storySet)}
      scanning={library.scanning}
      indexProgress={library.indexProgress}
      {previewFilter}
      storyDirs={storyState.storyDirs}
      focusOn={layouts[currentMode].focus}
      catalogContent={modalState.catalogContent}
      importDir={importState.importDir}
      onRescan={libraryController.rescan}
      onRescanDir={libraryController.rescanDir}
      onTidyFolder={(/** @type {string} */ p) => tidyState.open(p)}
      onRevealDir={libraryController.revealDir}
      onAddLocation={libraryController.indexRoot}
      onRemoveLibrary={libraryController.removeLibrary}
      onSetImportDir={importCtrl.setImportDir}
      onOpenNote={() => modalState.openCatalog()}
      onTogglePreview={() => modeCtrl.togglePreviewFilter()}
      onToggleSidebar={sidebarCtrl.toggleSidebar}
      onToggleFocus={modeCtrl.toggleFocusMode}
      onToggleAppearance={toggleAppearance}
      onShowShortcuts={() => modalState.openShortcuts()}
      onShowSettings={settingsCtrl.openSettings}
      onCatalogChange={modalState.catalogEdited}
      garden={gardenAccount}
      onGardenSignIn={settingsCtrl.gardenSignIn}
      onGardenSignOut={settingsCtrl.gardenSignOut}
      onOpenUrl={settingsCtrl.openUrl}
      onMovePhotos={libraryController.movePhotos}
      selectedCount={selection.paths.size || (view[sel] ? 1 : 0)}
      onMoveSelectedPhotos={libraryController.moveSelectedPhotosToDir}
      onRenameDir={libraryController.renameDir}
      onCreateFolder={libraryController.createFolder}
      onMoveDir={libraryController.moveDir}
      onDevelopStory={storyCtrl.exportLocalStory}
      onPublishStory={storyCtrl.publishStory}
      onExportLocalStory={storyCtrl.exportLocalStory}
      publishing={!!activity.progress}
      publishStatus={status}
      storyPublished={storyState.storyPublished}
    />
  {/snippet}

  <div class="body">
    {#if sidebarVisible}
      {@render sidebarPanel(false, () => {})}
    {/if}
    {#if sidebarPeek && !sidebarVisible}
      <div
        class="sidebar-peek"
        role="region"
        aria-label="Folder browser preview"
        onmouseenter={sidebarCtrl.openPeek}
        onmouseleave={sidebarCtrl.schedulePeekClose}
      >
        {@render sidebarPanel(true, sidebarCtrl.closePeek)}
      </div>
    {/if}
    <div class="content" role="presentation" onmousedown={onStartWindowDrag}>
      <CullTopRail
        {sidebarVisible}
        onToggleSidebar={sidebarCtrl.toggleSidebar}
        onOpenSidebarPeek={sidebarCtrl.openPeek}
        onScheduleSidebarPeekClose={sidebarCtrl.schedulePeekClose}
        focusOn={layouts[currentMode].focus}
        onToggleFocus={modeCtrl.toggleFocusMode}
        onToggleAppearance={toggleAppearance}
        onShowShortcuts={() => modalState.openShortcuts()}
        curDir={library.curDir}
        rootDir={library.root}
        {dirLabel}
        minRating={cullingState.minRating}
        onSetMinRating={cullingCtrl.setMinRating}
        textFilter={cullingState.textFilter}
        onSetTextFilter={cullingCtrl.setTextFilter}
        pickFilter={cullingState.pickFilter}
        onSetPickFilter={cullingCtrl.setPickFilter}
        filterStory={storyState.filterStory}
        onToggleFilterStory={() => (storyState.filterStory = !storyState.filterStory)}
        sortDesc={cullingState.sortDesc}
        onSetSortDesc={(desc) => {
          cullingState.sortDesc = desc;
          cullingCtrl.saveGridPrefs();
          if (applePhotos.active) navigationCtrl.handleOpenApplePhotos(applePhotos.album);
        }}
        viewLength={view.length}
        framesLength={library.frames.length}
        applePhotosActive={applePhotos.active}
        applePhotosTotal={applePhotos.currentTotal}
        applePhotosOffset={applePhotos.offset}
        applePhotosBusy={applePhotos.busy}
        onLoadMoreApplePhotos={() => loadMoreApplePhotos(cullingState.sortDesc)}
        immichActive={immich.active}
        immichTotal={immich.currentTotal}
        immichOffset={immich.offset}
        immichBusy={immich.busy}
        onLoadMoreImmich={() => loadMoreImmich(cullingState.sortDesc)}
        hasLibrary={!!library.root}
        {isTauri}
        scanning={library.scanning}
        onIndexRoot={libraryController.indexRoot}
        cards={importState.cards}
        importingCard={importState.importingCard}
        onImportCard={importCtrl.importCard}
        ejectableCard={importState.ejectableCard}
        ejecting={importState.ejecting}
        onEjectCard={importCtrl.ejectCard}
        onStopImport={importCtrl.stopImport}
        onStopCull={cullingCtrl.stopCull}
        publishedUrl={storyState.publishedUrl}
        hasPhotosToExport={!!((library.curDir || library.folder) && view.length)}
        selectionCount={selection.paths.size}
        onExportSelection={exportCtrl.exportSelection}
        layout={cullingState.layout}
        onToggleLayout={cullingCtrl.toggleLayout}
        cols={cullingState.cols}
        onSetCols={(n) => {
          cullingState.cols = n;
          cullingCtrl.saveGridPrefs();
        }}
        cellAspect={cullingState.cellAspect}
        onSetCellAspect={(a) => {
          cullingState.cellAspect = a;
          cullingCtrl.saveGridPrefs();
        }}
        fillCells={cullingState.fillCells}
        onToggleFillCells={() => {
          cullingState.fillCells = !cullingState.fillCells;
          cullingCtrl.saveGridPrefs();
        }}
        marginScale={cullingState.marginScale}
        onSetMarginScale={(m) => {
          cullingState.marginScale = m;
          cullingCtrl.saveGridPrefs();
        }}
        storySetSize={storyState.storySet.size}
        gardenSignedIn={!!gardenAccount?.signed_in}
        onPublishStory={storyCtrl.publishStory}
        publishVerb={storyState.publishVerb}
        canCullFolder={!applePhotos.active && !!((library.curDir || library.folder) && view.length)}
        onCullFolder={cullingCtrl.cullCurrentFolder}
        {masonryTooBig}
        {masonryLimit}
      />

    {#if previewFilter}
      <StoryView
        {view}
        storySet={storyState.storySet}
        storyContent={storyState.storyContent}
        progress={activity.progress}
        liveUrl={storyState.liveUrl ?? undefined}
        {thumbUrl}
        saveStoryContent={storyCtrl.saveStoryContent}
      />
    {:else}
      <CullView
        {view}
        loading={library.loading}
        {sel}
        selectedPaths={selection.paths}
        storySet={storyState.storySet}
        layout={cullingState.layout}
        cols={cullingState.cols}
        marginScale={cullingState.marginScale}
        cellAspect={cullingState.cellAspect}
        fillCells={cullingState.fillCells}
        progress={activity.progress}
        bind:currentScrollTop
        curDir={library.curDir}
        minRating={cullingState.minRating}
        {isTauri}
        {debug}
        selectGridItem={(/** @type {number} */ i, /** @type {MouseEvent | undefined} */ e) =>
          selectGridItem(view, i, e)}
        {openPhoto}
        openPhotoMenu={photoMenuCtrl.openPhotoMenu}
        toggleStoryWithPath={storyCtrl.toggleStoryWithPath}
        onPhotoDragStart={libraryController.onPhotoDragStart}
        closePhotoMenu={photoMenuCtrl.closePhotoMenu}
        hasRoot={!!library.root || applePhotos.active}
        applePhotosActive={applePhotos.active}
        scanning={library.scanning}
        onAddLibraryFolder={libraryController.indexRoot}
        {gridProseByRow}
        onSaveProse={storyCtrl.saveGridProse}
      />
    {/if}
    </div>
  </div>

  <ContextMenu
    photoMenu={photoMenuCtrl.photoMenu}
    selectedPaths={selection.paths}
    {installedEditors}
    copiedRecipe={developState.copiedRecipe}
    storySet={storyState.storySet}
    gardenUrl={storyState.publishedUrl}
    storyPublished={storyState.storyPublished}
    {stem}
    onClose={photoMenuCtrl.closePhotoMenu}
    onOpenPhoto={(/** @type {string} */ p) => openPhoto(p)}
    onOpenPreview={(/** @type {string} */ p) => openPhotoPreview(p)}
    onRevealInFinder={(/** @type {string} */ p) => revealPhotoInFinder(p)}
    onOpenInEditor={(/** @type {string} */ p, /** @type {string} */ app) => openPhotoInEditor(p, app)}
    onCopyImage={(/** @type {string} */ p) => copyImageToClipboard(p)}
    onCopySettings={copySettings}
    onPasteSettings={pasteSettings}
    onRate={(/** @type {number} */ n) => cullingCtrl.rate(n)}
    onToggleStory={(/** @type {string} */ p) => storyCtrl.toggleStoryWithPath(p)}
    onExportSelection={exportCtrl.exportSelection}
    onDevelopToVault={preferences.obsidian_enabled ? (/** @type {string} */ p) => photoMenuCtrl.developFromMenu(p, true) : undefined}
    onCull={applePhotos.active ? undefined : cullingCtrl.cullCurrentFolder}
    onPublishStory={storyCtrl.publishStory}
    onOpenGardenUrl={() => { if (storyState.publishedUrl) invoke("open_path", { path: storyState.publishedUrl }); }}
  />
</div>

<style>
  .cull {
    height: 100vh;
    display: flex;
    flex-direction: column;
    position: relative;
    z-index: 1;
    overflow: hidden;
  }
  .cull > .body {
    flex: 1;
    min-height: 0;
    display: flex;
    position: relative;
    z-index: 1;
    overflow: hidden;
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
    gap: 0;
    position: relative;
  }
  .sidebar-peek {
    position: absolute;
    top: 48px;
    left: 8px;
    z-index: 19;
    filter: drop-shadow(0 6px 16px rgba(0, 0, 0, 0.24));
    animation: sidebar-peek-in var(--duration-fast) var(--ease-soft);
  }
  @keyframes sidebar-peek-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
</style>
