import { tick, untrack } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen as tauriListen, emit } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  isTauri,
  applyFolderTheme,
  session,
  openFolderSession,
  activity, notify, hold, dismiss,
  startActivity, updateActivity, setActive, releaseActive, setQueueOpen, setProgress, patchProgress, advanceProgress,
  selection, positionIn, anchorIn, focusAt, selectOnly, selectGridItem,
  selectRange, toggle as toggleSelected, selectAll, clearSelection,
  setSelection, setAnchor, selectedFrames,
  virtualCollections,
} from "@modules/core";
import {
  libraryState,
  library,
  beginOpen,
  refreshFrames,
  clearFrames,
  refreshLoadedFrames,
  leaveFolder,
  setRoots,
  setDirs,
  setCurDir,
  setLoading,
  dirLabel,
  createLibraryController,
  createNavigationController,
  withPreviewVersions as opWithPreviewVersions,
  freshPreviewVersion as opFreshPreviewVersion,
  refreshDirs as opRefreshDirs,
} from "../library/index.js";
import { modalState } from "../modals/modalState.svelte.js";
import { initWhatsNew } from "../modals/modalOperations.js";
import { createCatalogController } from "../modals/catalogController.js";
import {
  applePhotos,
  initApplePhotos,
  connectApplePhotos,
  refreshApplePhotos,
  openApplePhotos,
  loadMoreApplePhotos,
  leaveApplePhotos,
  cancelApplePhotosTransfer,
} from "../apple-photos/applePhotos.svelte.js";
import { APPLE_PHOTOS_ROOT } from "../apple-photos/applePhotosTree.js";
import {
  immich,
  loadImmichCollections,
  openImmich,
  loadMoreImmich,
  leaveImmich,
} from "../immich/immich.svelte.js";
import { IMMICH_ROOT } from "../immich/immichTree.js";
import {
  exportState,
  saveExportPrefs,
  chooseExportFolder,
} from "../export/exportState.svelte.js";
import {
  cancelExportQueue,
} from "../export/exportOperations.js";
import {
  createExportController,
} from "../export/exportController.js";
import {
  importState,
} from "../import/importState.svelte.js";
import {
  createImportController,
} from "../import/importController.js";
import {
  closeMainWindow,
  minimizeMainWindow,
  zoomMainWindow,
  startWindowDrag,
  toggleAppearance as opToggleAppearance,
  hideWindow as opHideWindow,
} from "../windows/windowControls.js";
import { fullscreenState } from "../windows/fullscreenState.svelte.js";
import { createFullscreenController } from "../windows/fullscreenController.js";
import { createPaletteController } from "../windows/paletteController.js";
import {
  cullingState,
  initCullingState,
  MASONRY_LIMIT,
} from "../culling/cullingState.svelte.js";
import {
  createCullingController,
} from "../culling/cullingController.js";
import {
  handleCullStarted,
  handleCullProgress,
  handleCullFinished,
  handleCullFailed,
} from "../culling/cullingOperations.js";
import {
  developState,
} from "../develop/developState.svelte.js";
import {
  PREVIEW_PX,
} from "../develop/developOperations.js";
import {
  createDevelopController,
} from "../develop/developController.js";
import {
  createPrefetcher,
  createWorkingParker,
  createRenderPump,
} from "../develop/developRunner.js";
import {
  createPhotoLoader,
  previewUrl,
  openingUrl,
} from "../develop/photoLoader.js";
import {
  storyState,
} from "../story/storyState.svelte.js";
import {
  storyTheme,
  syncFolderTheme,
} from "../story/storyTheme.svelte.js";
import {
  parseStory,
  serializeStory,
} from "../story/storyParser.js";
import {
  refreshStoryDirs as opRefreshStoryDirs,
  watchPublishStatus,
  buildGridProse,
} from "../story/storyOperations.js";
import {
  createStoryController,
} from "../story/storyController.js";
import {
  settingsState,
  DEFAULT_PREFERENCES,
} from "../settings/settingsState.svelte.js";
import {
  loadPreferences as opLoadPreferences,
} from "../settings/settingsOperations.js";
import {
  createSettingsController,
} from "../settings/settingsController.js";
import { tidyState } from "../tidy/tidyState.svelte.js";
import { checkForUpdate, takeWhatsNew } from "@modules/updates";
import { createPhotoMenuController } from "../menus/photoMenuController.svelte.js";
import { createSidebarController } from "../sidebar/sidebarController.svelte.js";
import { createModeController } from "./modeController.svelte.js";
import { createExternalEditorsController } from "./externalEditors.svelte.js";
import { setupWindowPresence } from "./surfaceController.js";
import { copyImageToClipboard as opCopyImageToClipboard } from "./clipboard.js";
import { startAppLifecycle } from "../ipc/appWiring.js";
import { createKeyboardController } from "../shortcuts/keyboardController.js";

/**
 * Creates the root application orchestrator.
 */
export function createAppController() {
  const DRAG_PX = 768;
  let gpuAvailable = $state(false);

  /** @param {boolean} live */
  function liveRenderPx(live) {
    if (!live) return PREVIEW_PX;
    return developState.developEngine === "rapid" && gpuAvailable ? PREVIEW_PX : DRAG_PX;
  }

  /**
   * @param {string} event
   * @param {(payload: any) => void} handler
   * @returns {Promise<() => void>}
   */
  function listen(event, handler) {
    return tauriListen(event, handler).catch((error) => {
      if (!import.meta.hot) console.error(`Could not register ${event}:`, error);
      return () => {};
    });
  }

  const modeCtrl = createModeController({
    session,
    isTauri,
    invoke,
    getLibrary: () => library,
    getFolderSession: () => folderSession,
    isApplePhotosActive: () => applePhotos.active,
    hold,
    closePhotoMenu: () => photoMenuCtrl?.closePhotoMenu?.(),
    scheduleWorkingRelease: () => workingParker?.scheduleWorkingRelease?.(),
    getCurrentFrame: () => view[sel],
    getPhotoPath: () => photoPath,
    getDevelopState: () => developState,
    openPhoto: (p) => openPhoto(p),
    scheduleRender: (px) => scheduleRender(px),
    previewPx: PREVIEW_PX,
  });

  const layouts = modeCtrl.layouts;
  const currentMode = $derived(modeCtrl.currentMode);
  const previewFilter = $derived(modeCtrl.previewFilter);
  const spaceLook = $derived(modeCtrl.spaceLook);
  const zoomMode = $derived(modeCtrl.zoomMode);
  const panning = $derived(modeCtrl.panning);
  const {
    saveLayouts,
    switchMode,
    togglePreviewFilter,
    cycleZoom,
    toggleFocusMode,
    onPhotoPointerDown,
    onPhotoPointerMove,
    onPhotoPointerUp,
  } = modeCtrl;

  $effect(() => {
    return initApplePhotos();
  });

  const applePhotosLibrary = applePhotos;

  /**
   * @param {string} [album=""]
   * @param {any} [opts={}]
   */
  const handleOpenApplePhotos = (album = "", opts = {}) => openApplePhotos(album, {
    ...opts,
    sortDesc: cullingState.sortDesc,
    getView: () => view,
    sel,
    selectionAnchor,
    currentMode,
    photoPath,
    onResetFilters: () => {
      cullingState.minRating = 0;
      storyState.filterStory = false;
      modeCtrl.previewFilter = false;
      storyState.storySet = new Set();
      currentScrollTop = 0;
    },
    onSwitchMode: (mode) => switchMode(/** @type {"dev" | "cull"} */ (mode)),
  });

  /** @param {string} album */
  const handleRefreshActive = (album) => handleOpenApplePhotos(album, { refresh: true });

  /**
   * @param {string} [album=""]
   * @param {any} [opts={}]
   */
  const handleOpenImmich = (album = "", opts = {}) => openImmich(album, {
    ...opts,
    preferences,
    sortDesc: cullingState.sortDesc,
    getView: () => view,
    sel,
    selectionAnchor,
    currentMode,
    photoPath,
    onResetFilters: () => {
      cullingState.minRating = 0;
      storyState.filterStory = false;
      modeCtrl.previewFilter = false;
      storyState.storySet = new Set();
      currentScrollTop = 0;
    },
    onSwitchMode: (mode) => switchMode(/** @type {"dev" | "cull"} */ (mode)),
  });

  $effect(() => {
    syncFolderTheme(library.curDir, {
      isTauri,
      invoke,
      getCurrentDir: () => library.curDir,
      applyTheme: applyFolderTheme,
    });
  });

  let scanning = $derived(library.scanning);
  let sourceOffline = $state(false);

  $effect(() => {
    return initWhatsNew({
      isTauri,
      isDev: import.meta.env.DEV,
      takeWhatsNew,
      checkForUpdate,
    });
  });

  /** @type {import('../core/types.js').GardenAccount | null} */
  let gardenAccount = $state(null);
  let preferences = $state({ ...DEFAULT_PREFERENCES });

  const immichLibrary = $derived(immich.library(preferences));
  let indexProgress = $derived(library.indexProgress);

  /** @type {string | null} */
  let photoPath = $state(null);
  /** @type {string | null} */
  let picked = $state(null);
  const currentRating = $derived(library.frames.find((f) => f.path === photoPath)?.rating ?? 0);
  /** @type {string | null} */
  let imgUrl = $state(null);
  let status = $state("");

  $effect(() => {
    void storyState.gardenUrl;
    return watchPublishStatus(library.dir, {
      isTauri,
      count: storyState.storySet.size,
      signedIn: !!gardenAccount?.signed_in,
    });
  });

  /** @type {Record<string, number>} */
  let scrollOffsets = $state({});
  const externalEditorsCtrl = createExternalEditorsController({
    invoke,
    notify,
    isTauri,
    getCurrentPhotoPath: () => view[sel]?.path,
  });
  const installedEditors = $derived(externalEditorsCtrl.installedEditors);
  const { loadExternalEditors, openInEditor } = externalEditorsCtrl;

  /** @type {string | null} */
  let publishTaskId = $state(null);
  let currentScrollTop = $state(0);
  /** @type {import("../core/stores/session.js").FolderSession | null} */
  let folderSession = $state(null);

  $effect(() => {
    const d = library.dir;
    if (d && folderSession?.dir === d && currentMode === "cull") {
      scrollOffsets[d] = currentScrollTop;
      folderSession.saveScroll(currentScrollTop);
    }
  });

  $effect(() => {
    if (currentMode === "dev" && photoPath) session.setLastPhoto(photoPath);
  });

  $effect(() => {
    if (!isTauri || typeof window === "undefined") return;
    return setupWindowPresence({
      invoke,
      onPresenceChange: (inside) => { modeCtrl.pointerInside = inside; },
    });
  });

  const paletteCtrl = createPaletteController({
    isTauri,
    emit: isTauri ? emit : undefined,
    getCurrentMode: () => currentMode,
    getLayouts: () => layouts,
    getSpaceLook: () => spaceLook,
    getPicked: () => picked,
    getPhotoPath: () => photoPath,
    getDevelopState: () => developState,
    getExportState: () => exportState,
    getFrames: () => library.frames,
    getInstalledEditors: () => installedEditors,
    getStatus: () => status,
    saveLayouts,
    switchMode: (m, opts) => switchMode(m, opts),
    setSpaceLook: (v) => { modeCtrl.spaceLook = v; },
  });
  const {
    sendDevStateToPanel,
    handleSyncPalettes,
    handleSyncDevPanelWindow,
    toggleDevPanel,
  } = paletteCtrl;

  $effect(() => {
    if (currentMode === "dev" && isTauri) {
      void [
        photoPath,
        picked,
        developState.recipe,
        developState.developEngine,
        developState.renderMs,
        status,
        installedEditors,
        exportState.edge,
        exportState.border,
        developState.films,
        developState.papers,
        developState.luts,
        developState.caption,
        developState.tags,
        currentRating,
        developState.histogram,
        developState.developPhotoPercent,
      ];
      sendDevStateToPanel();
    }
  });

  $effect(() => {
    if (isTauri) {
      void [currentMode, layouts.dev.devPanel, layouts.dev.detached, spaceLook];
      handleSyncPalettes();
    }
  });

  const sidebarCtrl = createSidebarController({
    getLayouts: () => layouts,
    getCurrentMode: () => currentMode,
    getRoot: () => library.root,
    isApplePhotosSupported: () => applePhotos.supported,
    saveLayouts,
  });
  const { toggleSidebar } = sidebarCtrl;

  function togglePresetPanel() {}
  function toggleLutPanel() {}

  const toggleAppearance = () => opToggleAppearance({ invoke, notify });
  const hideWindow = () => opHideWindow({ invoke });

  const freshPreviewVersion = (/** @type {string} */ path) => opFreshPreviewVersion(path, { invoke });
  const refreshStoryDirs = async () => {
    if (isTauri && library.dirs.length) await opRefreshStoryDirs(library.dirs);
  };
  const refreshDirs = (light = false) =>
    opRefreshDirs({
      invoke,
      refreshStoryDirs,
      log: (msg) => /** @type {import('../core/types.js').RevealWindow} */ (window).__log?.(msg),
      light,
    });

  const masonryTooBig = $derived(library.frames.length > MASONRY_LIMIT);

  /** @type {import('../core/types.js').Frame[]} */
  const view = $derived.by(() => {
    let rows = library.frames;

    // 1. Virtual Collection Filter
    if (cullingState.activeVirtualCollectionId) {
      const vCol = virtualCollections.get(cullingState.activeVirtualCollectionId);
      if (vCol) {
        rows = virtualCollections.filterFrames(vCol, rows, storyState.storySet);
      }
    }

    // 2. Rating Filter
    if (cullingState.minRating > 0) {
      rows = rows.filter((frame) => (frame.rating ?? 0) >= cullingState.minRating);
    }

    // 3. Pick Filter
    if (cullingState.pickFilter === "picks") {
      rows = rows.filter((f) => f.pick === "picked");
    } else if (cullingState.pickFilter === "rejected") {
      rows = rows.filter((f) => f.pick === "rejected");
    } else if (cullingState.pickFilter === "unflagged") {
      rows = rows.filter((f) => !f.pick || f.pick === "none");
    }

    // 4. Story Filter
    if (storyState.filterStory) {
      rows = rows.filter((f) => storyState.storySet.has(stem(f.path.split("/").pop() || "")));
    }

    // 5. Search Text Filter (filename, stem, caption, tags)
    if (cullingState.textFilter && cullingState.textFilter.trim()) {
      const q = cullingState.textFilter.trim().toLowerCase();
      rows = rows.filter((f) => {
        const name = (f.name || f.path.split("/").pop() || "").toLowerCase();
        if (name.includes(q)) return true;
        if (f.caption && f.caption.toLowerCase().includes(q)) return true;
        if (Array.isArray(f.tags) && f.tags.some((/** @type {any} */ t) => String(t).toLowerCase().includes(q))) return true;
        return false;
      });
    }

    // 6. Sorting
    if (cullingState.sortDesc && !applePhotos.active) {
      rows = [...rows].sort(
        (a, b) => Number(b.capture_at || 0) - Number(a.capture_at || 0) || (a.path.split("/").pop() || "").localeCompare(b.path.split("/").pop() || ""),
      );
    }
    return rows;
  });

  const sel = $derived(positionIn(view));
  const selectionAnchor = $derived(anchorIn(view));

  const exportCtrl = createExportController({
    getView: () => view,
    getSel: () => sel,
    getSelection: () => selection,
    getPhotoPath: () => photoPath,
    getRecipe: () => developState.recipe,
    getPicked: () => picked,
    exportState,
    selectedFrames,
  });
  const {
    exportGrid,
    exportSelection,
    exportCurrent,
    exportToDailyNote,
    exportSelectionToDailyNote,
  } = exportCtrl;

  const photoMenuCtrl = createPhotoMenuController({
    invoke,
    getView: () => view,
    getSelection: () => selection,
    focusAt,
    selectOnly,
    hold,
    notify,
    openPhoto: (p, opts) => openPhoto(p, opts),
    getPreferences: () => preferences,
    exportSelectionToDailyNote: (p) => exportSelectionToDailyNote(p),
    exportState,
    startActivity,
    updateActivity,
    releaseActive,
    setProgress,
    patchProgress,
    activity,
  });
  const {
    openPhotoMenu,
    closePhotoMenu,
    revealPhotoInFinder,
    openPhotoPreview,
    openPhotoInEditor,
    developFromMenu,
  } = photoMenuCtrl;

  const settingsCtrl = createSettingsController({
    invoke,
    emit,
    isTauri,
    WebviewWindow,
    getPreferences: () => preferences,
    setPreferences: (v) => { preferences = v; },
    settingsState,
    exportState,
    saveExportPrefs,
    initCullingState,
    setGardenAccount: (acc) => { gardenAccount = acc; },
  });

  const {
    sendSettingsToPanel,
    openSettings,
    saveSettingsFromPanel,
    gardenSignIn,
    gardenSignOut,
    openUrl,
    choosePreferenceFolder,
  } = settingsCtrl;

  const fullscreenCtrl = createFullscreenController({
    getLibraryFrames: () => library.frames,
    getCurrentMode: () => currentMode,
    getPhotoPath: () => photoPath,
    getImgUrl: () => imgUrl,
    previewUrl,
    getView: () => view,
    getSel: () => sel,
    getLayouts: () => layouts,
    onSyncDevPanel: handleSyncDevPanelWindow,
  });
  const {
    prepareFullscreenFrame,
    enterFullscreen,
    exitFullscreen,
    toggleFullscreen,
  } = fullscreenCtrl;

  const libraryController = createLibraryController({
    invoke,
    notify,
    hold,
    dismiss,
    refreshDirs,
    openDir: (d, rm, rs, kf) => openDir(d, rm, rs, kf),
    state: libraryState,
    library,
    getSelectionPaths: () => selection.paths,
    getCurrentPhotoPath: () => view[sel]?.path,
    clearSelection,
    setProgress,
    startActivity,
    updateActivity,
    releaseActive,
    activity,
    isApplePhotosActive: () => applePhotos.active,
    refreshApplePhotos,
    handleRefreshActive,
    log: (msg) => /** @type {import('../core/types.js').RevealWindow} */ (window).__log?.(msg),
  });

  const {
    indexRoot,
    listLibraries,
    removeLibrary,
    rescanLibrary,
    rescan,
    rescanDir,
    revealDir,
    onPhotoDragStart,
    movePhotos,
    moveSelectedPhotosToDir,
    renameDir,
    createFolder,
    moveDir,
  } = libraryController;

  let debug = $state("");
  const withPreviewVersions = (/** @type {any[]} */ rows) => opWithPreviewVersions(rows, { invoke, isTauri });

  const navigationCtrl = createNavigationController({
    invoke,
    session,
    cullingState,
    storyState,
    beginOpen,
    withPreviewVersions,
    openFolderSession,
    scrollOffsets,
    switchMode: (m) => switchMode(m),
    refreshStory: () => refreshStory(),
    focusAt,
    selectOnly,
    openPhoto: (p, opts) => openPhoto(p, opts),
    layouts,
    getView: () => view,
    leaveApplePhotos,
    leaveImmich,
    handleOpenApplePhotos,
    handleOpenImmich,
    setFolderSession: (s) => { folderSession = s; },
    setCurrentScrollTop: (pos) => { currentScrollTop = pos; },
    setPreviewFilter: (v) => { modeCtrl.previewFilter = v; },
    setLoading,
    setCurDir,
    setDebug: (d) => { debug = d; },
    isMasonryTooBig: () => masonryTooBig,
    applePhotosRoot: APPLE_PHOTOS_ROOT,
    immichRoot: IMMICH_ROOT,
    log: (msg) => /** @type {import('../core/types.js').RevealWindow} */ (window).__log?.(msg),
  });
  const { openDir, openFolder, pickFolder } = navigationCtrl;

  /** @param {string} name */
  const stem = (name) => name.replace(/\.[^.]+$/, "");

  const catalogCtrl = createCatalogController({
    invoke,
    modalState,
    getRoot: () => library.root,
  });
  const { loadCatalog, catalogEdited } = catalogCtrl;

  $effect(() => {
    if (library.root) {
      loadCatalog();
    }
  });

  const storyCtrl = createStoryController({
    getDir: () => library.dir,
    getView: () => view,
    getSel: () => sel,
    getCols: () => cullingState.cols,
    getSignedIn: () => gardenAccount?.signed_in,
    getExportState: () => exportState,
    onRefreshed: () => refreshStoryDirs(),
  });
  const {
    loadStory,
    saveStoryContent,
    saveGridProse,
    refreshStory,
    toggleStoryWithPath,
    toggleStory,
    publishStory,
    exportLocalStory,
  } = storyCtrl;

  const gridProseByRow = $derived.by(() =>
    buildGridProse(parseStory(storyState.storyContent).blocks, view, cullingState.cols),
  );

  const cullingCtrl = createCullingController({
    session,
    getView: () => view,
    getFrames: () => library.frames,
    getCurrentDir: () => library.curDir,
    openDir: (d, rm, rs, kf) => openDir(d, rm, rs, kf),
    loadStory: () => loadStory(),
    refreshStory: () => refreshStory(),
    refreshStoryDirs: () => refreshStoryDirs(),
    isApplePhotosActive: () => applePhotos.active,
  });
  const {
    saveGridPrefs,
    toggleLayout,
    rate,
    setMinRating,
    triggerAiCull,
    cullCurrentFolder,
    stopCull,
  } = cullingCtrl;

  const importCtrl = createImportController({
    invoke,
    notify,
    importState,
    getArchiveDir: () => library.root,
    refreshDirs: () => refreshDirs(),
    openDir: (folder) => openDir(folder),
  });
  const {
    toggleAutoImport,
    importCard,
    pollCards,
    setImportDir,
    stopImport,
    ejectCard,
    handleCardMounted,
    handleCardUnmounted,
  } = importCtrl;

  /** @param {string} path */
  async function copyImageToClipboard(path) {
    await opCopyImageToClipboard(path, {
      currentMode,
      photoPath,
      developState,
      invoke,
      notify,
      previewPx: PREVIEW_PX,
    });
  }

  const { prefetchNeighbours, warmSelection } = createPrefetcher({
    isTauri,
    invoke,
    getPhotoPath: () => photoPath,
    getView: () => view,
    getSel: () => sel,
    getCurrentMode: () => currentMode,
  });

  $effect(() => {
    const path = view[sel]?.path;
    if (path) untrack(() => warmSelection(path));
  });

  const workingParker = createWorkingParker({
    isTauri,
    invoke,
    getCurrentMode: () => currentMode,
    getPhotoPath: () => photoPath,
  });
  const { scheduleWorkingPark, scheduleWorkingRelease } = workingParker;

  const renderPump = createRenderPump({
    developState,
    invoke,
    tick,
    getPhotoPath: () => photoPath,
    isDockedCrop: () => Boolean(developState.recipe) && Boolean(layouts?.dev?.devPanel) && !layouts?.dev?.detached && developState.dockedActiveTab === "crop",
    library,
    freshPreviewVersion,
    refreshFrames,
    onLoupeBlobCreated: (url) => {
      if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
      imgUrl = url;
    },
    setStatus: (s) => { status = s; },
  });
  const { scheduleRender } = renderPump;

  const photoLoader = createPhotoLoader({
    invoke,
    developState,
    getFrames: () => library.frames,
    getPhotoPath: () => photoPath,
    setPhotoPath: (p) => { photoPath = p; },
    setPicked: (p) => { picked = p; },
    getImgUrl: () => imgUrl,
    setImgUrl: (url) => { imgUrl = url; },
    setStatus: (s) => { status = s; },
    getPreferences: () => preferences,
    prefetchNeighbours,
    scheduleWorkingPark,
    scheduleRender,
    switchMode: (m, opts) => switchMode(m, opts),
    setSpaceLook: (v) => { modeCtrl.spaceLook = v; },
    getCurrentMode: () => currentMode,
    notify,
    previewPx: PREVIEW_PX,
  });
  const { openPhoto } = photoLoader;

  const devController = createDevelopController({
    state: developState,
    getPhotoPath: () => photoPath,
    getCurrentMode: () => currentMode,
    liveRenderPx,
    scheduleRender,
    invoke,
    emit: isTauri ? emit : undefined,
    hold,
    notify,
    isTauri,
    library,
    freshPreviewVersion,
    refreshFrames,
    previewUrl,
    getImgUrl: () => imgUrl,
    setImgUrl: (next) => {
      imgUrl = next;
    },
    sendDevStateToPanel,
    layouts,
    saveLayouts,
    saveExportPrefs,
    getSourceFrame: () => view[sel],
    getSelectedFrames: () => selectedFrames(view),
    activity,
    startActivity,
    updateActivity,
    releaseActive,
    setProgress,
    patchProgress,
    advanceProgress,
    previewPx: PREVIEW_PX,
    onUpdateLoupeUrl: (/** @type {any} */ bytes) => {
      if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
      imgUrl = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
    },
  });

  const {
    edited,
    dockedEdited,
    captionEdited,
    tagsEdited,
    undoRecipeEdit,
    redoRecipeEdit,
    clearDevelopment,
    setDevNum,
    resetOne,
    addLutLayer,
    removeLutLayer,
    updateLutOpacity,
    setLutFile,
    applyEngineChange,
    dockedEngineChanged,
    applyResetRecipe,
    toggleCheckLayer,
    dockedToggleClipping,
    dockedToggleCaptionOverlay,
    dockedHidePanel,
    dockedExportSettingsChanged,
    copySettings,
    applyRecipeToFrames,
    pasteSettings,
  } = devController;

  const keyboardCtrl = createKeyboardController({
    getState: () => ({
      currentMode,
      fullscreen: fullscreenState.active,
      spaceLook,
      previewFilter,
      devPanel: layouts.dev.devPanel,
      view,
      sel,
      photoPath,
      recipe: developState.recipe,
      lastEditedKey: developState.lastEditedKey,
      developEngine: developState.developEngine ?? "spektra",
      developPhotoPercent: developState.developPhotoPercent,
      cols: cullingState.cols,
      marginScale: cullingState.marginScale,
      zoneMaskPreview: developState.zoneMaskPreview,
      dockedActiveZone: developState.dockedActiveZone,
      selectionAnchor,
    }),
    actions: {
      switchMode,
      setSpaceLook: (v) => { modeCtrl.spaceLook = v; },
      saveLayouts,
      toggleSidebar,
      exitFullscreen,
      toggleFullscreen,
      togglePreviewFilter,
      toggleFocusMode,
      toggleAppearance,
      toggleLayout,
      toggleStory,
      toggleShortcuts: () => { modalState.toggleShortcuts(); },
      toggleDevPanel,
      cycleZoom,
      selectAll,
      copyImageToClipboard,
      clearSelection,
      undoRecipeEdit,
      redoRecipeEdit,
      exportSelection,
      copySettings,
      pasteSettings,
      setZoneMask: (mask) => { developState.zoneMaskPreview = mask; },
      setPhotoSize: (size) => {
        developState.developPhotoPercent = size;
        session.setPhotoSize(developState.developPhotoPercent);
        sendDevStateToPanel();
      },
      setCols: (n) => {
        cullingState.cols = n;
        saveGridPrefs();
      },
      setMarginScale: (m) => {
        cullingState.marginScale = m;
        saveGridPrefs();
      },
      updateRecipe: (newRecipe) => {
        developState.recipe = newRecipe;
        developState.developEngine = newRecipe.engine;
        edited(true);
        sendDevStateToPanel();
      },
      focusAt,
      selectRange,
      selectOnly,
      prepareFullscreenFrame,
      openPhoto: (p) => openPhoto(p),
      rate,
      toggleSelected,
      setAnchor,
    },
  });
  const { onKey } = keyboardCtrl;

  $effect(() => {
    return startAppLifecycle({
      isTauri,
      listen,
      invoke,
      session,
      layouts,
      saveLayouts,
      modeCtrl,
      importCtrl,
      libraryController,
      navigationCtrl,
      cullingCtrl,
      exportCtrl,
      settingsCtrl,
      photoMenuCtrl,
      externalEditorsCtrl,
      devController,
      keyboardCtrl,
      paletteCtrl,
      modalState,
      libraryState,
      library,
      cullingState,
      developState,
      exportState,
      importState,
      settingsState,
      preferences,
      initCullingState,
      loadPreferences: opLoadPreferences,
      getCurrentMode: () => currentMode,
      switchMode: (m, opts) => switchMode(m, opts),
      refreshDirs,
      rescan,
      getPublishTaskId: () => publishTaskId,
      getSourceOffline: () => sourceOffline,
      setSourceOffline: (v) => { sourceOffline = v; },
      getPhotoPath: () => photoPath,
      getCurrentFramePath: () => view[sel]?.path,
      getView: () => view,
      getSel: () => sel,
      selectedFrames,
      setGpuAvailable: (v) => { gpuAvailable = v; },
      setGardenAccount: (acc) => { gardenAccount = acc; },
      setQueueOpen,
      scheduleRender,
      openPhoto: (p) => openPhoto(p),
      chooseExportFolder: (cb) => chooseExportFolder(cb),
      togglePresetPanel,
      toggleLutPanel,
      log: (msg) => /** @type {import('../core/types.js').RevealWindow} */ (window).__log?.(msg),
    });
  });

  return {
    get currentMode() { return currentMode; },
    get view() { return view; },
    get sel() { return sel; },
    get photoPath() { return photoPath; },
    get developState() { return developState; },
    get sourceOffline() { return sourceOffline; },
    get currentScrollTop() { return currentScrollTop; },
    set currentScrollTop(v) { currentScrollTop = v; },
    get onKey() { return onKey; },
    isTauri,
    applePhotos,
    cancelApplePhotosTransfer,
    dismissApplePhotosTransfer: () => { applePhotos.transfer = null; },
    fullscreenState,
    closeMainWindow,
    minimizeMainWindow,
    zoomMainWindow,
    exitFullscreen,

    get cullProps() {
      return {
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
        get currentScrollTop() { return currentScrollTop; },
        set currentScrollTop(v) { currentScrollTop = v; },
        currentMode,
        layouts,
        isTauri,
        masonryTooBig,
        masonryLimit: MASONRY_LIMIT,
        dirLabel,
        installedEditors,
        developState,
        gridProseByRow,
        onStartWindowDrag: startWindowDrag,
        toggleAppearance,
        copyImageToClipboard,
        copySettings,
        pasteSettings,
        selectGridItem: (/** @type {any[]} */ v, /** @type {number} */ i, /** @type {MouseEvent | undefined} */ e) => selectGridItem(v, i, e),
        loadMoreApplePhotos: (/** @type {boolean} */ desc) => loadMoreApplePhotos(desc),
        loadMoreImmich: (/** @type {boolean} */ desc) => loadMoreImmich(desc),
        connectApplePhotos: () => connectApplePhotos(),
        refreshApplePhotos: (/** @type {any} */ onAct) => refreshApplePhotos(onAct),
        handleRefreshActive,
        loadImmichCollections: (/** @type {any} */ p, /** @type {boolean | undefined} */ r) => loadImmichCollections(p, r),
        openPhoto: (/** @type {string} */ p, /** @type {any} */ opts) => openPhoto(p, opts),
        openPhotoPreview: (/** @type {string} */ p) => openPhotoPreview(p),
        revealPhotoInFinder: (/** @type {string} */ p) => revealPhotoInFinder(p),
        openPhotoInEditor: (/** @type {string} */ p, /** @type {string} */ app) => openPhotoInEditor(p, app),
      };
    },

    get developProps() {
      return {
        developState,
        exportState,
        layouts,
        saveLayouts,
        session,
        isTauri,
        photoPath,
        picked,
        imgUrl,
        status,
        currentRating,
        sourceOffline,
        installedEditors,
        zoomMode: modeCtrl.zoomMode,
        panning: modeCtrl.panning,
        onPhotoPointerDown,
        onPhotoPointerMove,
        onPhotoPointerUp,
        onCropChange: (/** @type {boolean} */ live) => edited(live),
        onStartWindowDrag: startWindowDrag,
        devController,
        exportCtrl,
        paletteCtrl,
        openInEditor,
      };
    },
  };
}
