/**
 * Mode and surface controller.
 *
 * Coordinates workflow mode transitions (Cull vs Develop), layout states,
 * zoom cycles, pan gestures, focus mode, and editorial preview filtering.
 */

import { nextZoomMode, createPanGesture } from "./surfaceController.js";

/**
 * @param {{
 *   session?: {
 *     lastMode?: () => any,
 *     setLastMode?: (mode: any) => void,
 *     setModeLayouts?: (layouts: any) => void,
 *   },
 *   isTauri?: boolean,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   getLibrary?: () => { dir?: string | null, curDir?: string | null, folder?: string | null },
 *   getFolderSession?: () => { dir?: string | null, saveMode?: (m: string) => void } | null,
 *   isApplePhotosActive?: () => boolean,
 *   hold?: (msg: string) => void,
 *   closePhotoMenu?: () => void,
 *   scheduleWorkingRelease?: () => void,
 *   getCurrentFrame?: () => { path: string } | null | undefined,
 *   getPhotoPath?: () => string | null,
 *   getDevelopState?: () => { recipe?: any, useCanvas?: boolean, developPhotoPercent?: number },
 *   openPhoto?: (path: string) => void,
 *   scheduleRender?: (px: number) => void,
 *   previewPx?: number,
 * }} deps
 */
export function createModeController({
  session,
  isTauri = false,
  invoke = async () => {},
  getLibrary = () => ({ dir: null, curDir: null, folder: null }),
  getFolderSession = () => null,
  isApplePhotosActive = () => false,
  hold = () => {},
  closePhotoMenu = () => {},
  scheduleWorkingRelease = () => {},
  getCurrentFrame = () => null,
  getPhotoPath = () => null,
  getDevelopState = () => ({ recipe: null, useCanvas: false, developPhotoPercent: 100 }),
  openPhoto = () => {},
  scheduleRender = () => {},
  previewPx = 2048,
}) {
  /** @type {"cull" | "dev"} */
  let currentMode = $state(/** @type {"cull" | "dev"} */ (session?.lastMode?.() ?? "cull"));
  let previewFilter = $state(false);
  let spaceLook = $state(false);
  /** @type {"frame" | "fill" | "actual"} */
  let zoomMode = $state("frame");
  let pointerInside = $state(false);
  let panning = $state(false);

  /** @type {Record<"cull" | "dev", Record<string, boolean>>} */
  const layouts = $state({
    cull: { sidebar: true, focus: false, devPanel: false },
    dev: { sidebar: false, focus: false, devPanel: true, detached: false },
  });

  function saveLayouts() {
    session?.setModeLayouts?.(layouts);
  }

  /**
   * @param {"cull" | "dev"} to
   * @param {{ openDevPanel?: boolean }} [opts]
   */
  async function switchMode(to, { openDevPanel = true } = {}) {
    closePhotoMenu();
    if (to !== "dev") {
      zoomMode = "frame";
      scheduleWorkingRelease();
    }
    if (to === "dev" && openDevPanel) {
      spaceLook = false;
      if (currentMode !== "dev") {
        layouts.dev.devPanel = true;
        saveLayouts();
      }
    }

    currentMode = to;
    session?.setLastMode?.(to);

    const folderSession = getFolderSession();
    const library = getLibrary();
    if (folderSession && folderSession.dir === library?.dir) {
      folderSession.saveMode?.(to);
    }

    const currentFocus = layouts[currentMode]?.focus;
    if (isTauri) {
      invoke("set_focus", { enabled: currentFocus }).catch(() => {});
    }

    if (to === "dev") {
      const currentFrame = getCurrentFrame();
      const photoPath = getPhotoPath();
      const developState = getDevelopState();

      if (currentFrame && (photoPath !== currentFrame.path || !developState.recipe)) {
        openPhoto(currentFrame.path);
      } else if (developState.useCanvas) {
        scheduleRender(previewPx);
      }
    }
  }

  /**
   * @param {boolean} [value]
   */
  function togglePreviewFilter(value) {
    const next = value ?? !previewFilter;
    if (next) {
      if (isApplePhotosActive()) {
        hold("Editorial needs a filesystem folder. You can edit and export Apple Photos directly.");
        return;
      }
      const lib = getLibrary();
      if (!lib.curDir && !lib.folder) return;
    }
    previewFilter = next;
  }

  /**
   * @param {boolean} [reverse]
   */
  function cycleZoom(reverse = false) {
    if (currentMode !== "dev") return;
    zoomMode = nextZoomMode(zoomMode, reverse);
  }

  async function toggleFocusMode() {
    const targetFocus = !layouts[currentMode].focus;
    layouts[currentMode].focus = targetFocus;
    saveLayouts();
    if (isTauri) {
      await invoke("set_focus", { enabled: targetFocus }).catch(() => {});
    }
  }

  const panGesture = createPanGesture({
    getZoomMode: () => zoomMode,
    getPhotoPercent: () => getDevelopState().developPhotoPercent ?? 100,
    onPanningChange: (p) => {
      panning = p;
    },
  });

  return {
    get currentMode() {
      return currentMode;
    },
    set currentMode(v) {
      currentMode = v;
    },
    get previewFilter() {
      return previewFilter;
    },
    set previewFilter(v) {
      previewFilter = v;
    },
    get spaceLook() {
      return spaceLook;
    },
    set spaceLook(v) {
      spaceLook = v;
    },
    get zoomMode() {
      return zoomMode;
    },
    set zoomMode(v) {
      zoomMode = v;
    },
    get pointerInside() {
      return pointerInside;
    },
    set pointerInside(v) {
      pointerInside = v;
    },
    get panning() {
      return panning;
    },
    layouts,
    saveLayouts,
    switchMode,
    togglePreviewFilter,
    cycleZoom,
    toggleFocusMode,
    onPhotoPointerDown: panGesture.onPhotoPointerDown,
    onPhotoPointerMove: panGesture.onPhotoPointerMove,
    onPhotoPointerUp: panGesture.onPhotoPointerUp,
  };
}
