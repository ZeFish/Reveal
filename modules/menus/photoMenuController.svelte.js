/**
 * Photo context menu controller and external launcher operations.
 *
 * Coordinates right-click frame context menu, Finder/Preview/Editor dispatching,
 * and direct export/daily note development actions.
 */

/**
 * @typedef {Object} PhotoMenuState
 * @property {any} frame
 * @property {number} x
 * @property {number} y
 */

/**
 * Creates a bound photo context menu controller.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   getView?: () => any[],
 *   getSelection?: () => { paths: Set<string> },
 *   focusAt?: (view: any[], index: number) => void,
 *   selectOnly?: (view: any[], index: number) => void,
 *   hold?: (msg: string) => void,
 *   notify?: (msg: string, ms?: number) => void,
 *   openPhoto?: (path: string, opts?: { openDevPanel?: boolean }) => Promise<any> | void,
 *   getPreferences?: () => { obsidian_enabled?: boolean },
 *   preferences?: { obsidian_enabled?: boolean },
 *   exportSelectionToDailyNote?: (path: string) => Promise<any> | void,
 *   exportState?: { folder: string, edge: number, border: boolean },
 *   startActivity?: (action: string, label: string, total: number) => string | null,
 *   updateActivity?: (...args: any[]) => void,
 *   releaseActive?: (id: any) => void,
 *   setProgress?: (p: any) => void,
 *   patchProgress?: (p: any) => void,
 *   activity?: any,
 * }} deps
 */
export function createPhotoMenuController(deps) {
  const {
    invoke,
    getView = () => [],
    getSelection = () => ({ paths: new Set() }),
    focusAt = () => {},
    selectOnly = () => {},
    hold = () => {},
    notify = () => {},
    openPhoto = async () => {},
    getPreferences,
    preferences = {},
    exportSelectionToDailyNote = async () => {},
    exportState = { folder: "", edge: 2048, border: false },
    startActivity = () => null,
    updateActivity = () => {},
    releaseActive = () => {},
    setProgress = () => {},
    patchProgress = () => {},
    activity = {},
  } = deps;

  /** @type {PhotoMenuState | null} */
  let photoMenu = $state(null);

  /**
   * @param {number} index
   * @param {MouseEvent} event
   * @param {any[]} [viewOverride]
   * @param {{ paths: Set<string> }} [selectionOverride]
   * @param {(view: any[], index: number) => void} [focusAtOverride]
   * @param {(view: any[], index: number) => void} [selectOnlyOverride]
   */
  function openPhotoMenu(index, event, viewOverride, selectionOverride, focusAtOverride, selectOnlyOverride) {
    event.preventDefault();
    const currentView = viewOverride || getView();
    const currentSelection = selectionOverride || getSelection();
    const currentFocusAt = focusAtOverride || focusAt;
    const currentSelectOnly = selectOnlyOverride || selectOnly;

    const frame = currentView[index];
    if (!frame) return;
    if (!currentSelection.paths.has(frame.path)) currentSelectOnly(currentView, index);
    currentFocusAt(currentView, index);
    photoMenu = {
      frame,
      x: event.clientX,
      y: event.clientY,
    };
  }

  function closePhotoMenu() {
    photoMenu = null;
  }

  /** @param {string} path */
  async function revealPhotoInFinder(path) {
    closePhotoMenu();
    try {
      await invoke("reveal_in_finder", { path });
    } catch (error) {
      hold(`Could not reveal photo: ${error}`);
    }
  }

  /** @param {string} path */
  async function openPhotoPreview(path) {
    closePhotoMenu();
    if (path.startsWith("apple-photos://")) {
      await openPhoto(path, { openDevPanel: false });
      return;
    }
    try {
      await invoke("open_path", { path });
    } catch (error) {
      hold(`Could not open photo: ${error}`);
    }
  }

  /**
   * @param {string} path
   * @param {string} appPath
   */
  async function openPhotoInEditor(path, appPath) {
    closePhotoMenu();
    try {
      await invoke("open_in_editor", { filePath: path, appPath });
    } catch (error) {
      hold(`Could not open editor: ${error}`);
    }
  }

  /**
   * @param {string} path
   * @param {boolean} toVault
   */
  async function developFromMenu(path, toVault) {
    closePhotoMenu();
    /** @type {string | null} */
    let devJobId = null;
    try {
      const sidecar = await invoke("load_sidecar", { path });
      if (!sidecar?.engine_settings) {
        notify("No development engine is active for this photo", 3000);
        return;
      }
      if (toVault) {
        const prefs = getPreferences ? getPreferences() : preferences;
        if (!prefs?.obsidian_enabled) {
          notify("Obsidian integration is disabled in settings", 3000);
          return;
        }
        await exportSelectionToDailyNote(path);
        return;
      }
      const filename = path.split("/").pop();
      setProgress({ verb: "Developing", done: 0, total: 1, current: filename });
      devJobId = startActivity("develop", `Developing ${filename}`, 1);
      await invoke("export_photo", {
        path,
        recipe: sidecar.engine_settings,
        destDir: exportState.folder,
        longEdge: exportState.edge,
        borderFrac: exportState.border ? 0.04 : 0,
      });
      patchProgress({ done: 1 });
      updateActivity(devJobId, { done: 1, phase: "Complete", status: "completed" });
    } catch (error) {
      hold(`Development failed: ${error}`);
      if (devJobId) updateActivity(devJobId, { phase: String(error), status: "failed" });
    } finally {
      if (devJobId) releaseActive(devJobId);
      if (activity.progress) {
        setTimeout(() => {
          setProgress(null);
        }, 1200);
      }
    }
  }

  return {
    get photoMenu() {
      return photoMenu;
    },
    set photoMenu(v) {
      photoMenu = v;
    },
    openPhotoMenu,
    closePhotoMenu,
    revealPhotoInFinder,
    openPhotoPreview,
    openPhotoInEditor,
    developFromMenu,
  };
}
