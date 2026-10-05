/**
 * Library Controller.
 *
 * Coordinates catalogue root indexation, rescanning, directory management,
 * and photo moving/dragging operations with UI state and notifications.
 */

import {
  indexRoot as opIndexRoot,
  listLibraries as opListLibraries,
  removeLibrary as opRemoveLibrary,
  rescanLibrary as opRescanLibrary,
  rescan as opRescan,
  rescanDir as opRescanDir,
  revealDir as opRevealDir,
  onPhotoDragStart as opOnPhotoDragStart,
  movePhotos as opMovePhotos,
  renameDir as opRenameDir,
  createFolder as opCreateFolder,
  moveDir as opMoveDir,
} from "./libraryOperations.js";

/**
 * Creates a bound library controller instance.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void,
 *   hold?: (msg: string) => void,
 *   dismiss?: () => void,
 *   refreshDirs: (light?: boolean) => Promise<boolean>,
 *   openDir: (dir: string, restoreMode?: boolean, restoreSession?: boolean, keepFilters?: boolean) => Promise<any> | void,
 *   state: any,
 *   library: any,
 *   getSelectionPaths?: () => Iterable<string>,
 *   getCurrentPhotoPath?: () => string | null | undefined,
 *   clearSelection?: () => void,
 *   setProgress?: (p: any) => void,
 *   startActivity?: (action: string, label: string, total: number) => any,
 *   updateActivity?: (...args: any[]) => void,
 *   releaseActive?: (id: any) => void,
 *   activity?: { message?: string },
 *   isApplePhotosActive?: () => boolean,
 *   refreshApplePhotos?: (cb?: any) => Promise<void>,
 *   handleRefreshActive?: any,
 *   log?: (msg: string) => void,
 * }} deps
 */
export function createLibraryController(deps) {
  const {
    invoke,
    notify = () => {},
    hold = () => {},
    dismiss = () => {},
    refreshDirs,
    openDir,
    state,
    library,
    getSelectionPaths = () => [],
    getCurrentPhotoPath = () => null,
    clearSelection = () => {},
    setProgress = () => {},
    startActivity = () => "",
    updateActivity = () => {},
    releaseActive = () => {},
    activity = {},
    isApplePhotosActive = () => false,
    refreshApplePhotos = async () => {},
    handleRefreshActive = () => {},
    log = () => {},
  } = deps;

  return {
    indexRoot: () =>
      opIndexRoot({
        invoke,
        notify,
        refreshDirs,
        openDir: (d) => openDir(d),
        state,
        library,
      }),

    listLibraries: () =>
      opListLibraries({
        invoke,
        log,
      }),

    removeLibrary: (/** @type {string} */ path) =>
      opRemoveLibrary(path, {
        invoke,
        notify,
        refreshDirs,
        curDir: library.curDir,
      }),

    rescanLibrary: (/** @type {string} */ path) =>
      opRescanLibrary(path, {
        invoke,
        refreshDirs,
        state,
      }),

    rescan: () =>
      opRescan({
        invoke,
        refreshDirs,
        openDir: (d, rm, rs, kf) => openDir(d, rm, rs, kf),
        state,
        library,
        isApplePhotosActive: isApplePhotosActive(),
        refreshApplePhotos,
        handleRefreshActive,
      }),

    rescanDir: (/** @type {string} */ path) =>
      opRescanDir(path, {
        invoke,
        refreshDirs,
        openDir: (d, rm, rs, kf) => openDir(d, rm, rs, kf),
        hold,
        dismiss,
        activityMessage: activity.message,
        state,
        roots: library.roots,
        curDir: library.curDir,
      }),

    revealDir: (/** @type {string} */ path) =>
      opRevealDir(path, { invoke, hold }),

    onPhotoDragStart: (/** @type {string} */ path, /** @type {DragEvent} */ event) =>
      opOnPhotoDragStart(path, event, {
        selectedPaths: new Set(getSelectionPaths()),
        hold,
      }),

    movePhotos: (/** @type {string[]} */ paths, /** @type {string} */ destDir) =>
      opMovePhotos(paths, destDir, {
        invoke,
        notify,
        hold,
        setProgress,
        startActivity,
        updateActivity,
        releaseActive,
        refreshDirs,
        curDir: library.curDir,
        openDir: (d, rm, rs, kf) => openDir(d, rm, rs, kf),
        clearSelection,
      }),

    moveSelectedPhotosToDir: (/** @type {string} */ destDir) => {
      const paths = [...getSelectionPaths()];
      const current = getCurrentPhotoPath();
      if (!paths.length && current) paths.push(current);
      if (paths.length) {
        return opMovePhotos(paths, destDir, {
          invoke,
          notify,
          hold,
          setProgress,
          startActivity,
          updateActivity,
          releaseActive,
          refreshDirs,
          curDir: library.curDir,
          openDir: (d, rm, rs, kf) => openDir(d, rm, rs, kf),
          clearSelection,
        });
      }
    },

    renameDir: (/** @type {string} */ path, /** @type {string} */ newName) =>
      opRenameDir(path, newName, {
        invoke,
        notify,
        refreshDirs,
        curDir: library.curDir,
        openDir: (d) => openDir(d),
      }),

    createFolder: (/** @type {string} */ parentDir, /** @type {string} */ name) =>
      opCreateFolder(parentDir, name, { invoke, notify }),

    moveDir: (/** @type {string} */ path, /** @type {string} */ destParentDir) =>
      opMoveDir(path, destParentDir, {
        invoke,
        notify,
        refreshDirs,
        curDir: library.curDir,
        openDir: (d) => openDir(d),
      }),
  };
}
