import { unpackFrame, withTimeout, RENDER_TIMEOUT_MS, PREVIEW_PX } from "./developOperations.js";

/**
 * Creates prefetching timers for neighbor and selection decodes.
 *
 * @param {Object} options
 * @param {boolean} options.isTauri
 * @param {(cmd: string, args?: any) => Promise<any>} options.invoke
 * @param {() => string | null} options.getPhotoPath
 * @param {() => any[]} options.getView
 * @param {() => number} options.getSel
 * @param {() => string} options.getCurrentMode
 */
export function createPrefetcher({
  isTauri,
  invoke,
  getPhotoPath,
  getView,
  getSel,
  getCurrentMode,
}) {
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let prefetchTimer;
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let selectionWarmTimer;

  /** @param {string} path */
  function prefetchNeighbours(path) {
    if (!isTauri) return;
    clearTimeout(prefetchTimer);
    prefetchTimer = setTimeout(() => {
      if (getPhotoPath() !== path) return;
      const view = getView();
      const i = view.findIndex((f) => f.path === path);
      if (i < 0) return;
      for (const n of [view[i + 1], view[i - 1]]) {
        if (n?.path) invoke("prefetch_photo", { path: n.path }).catch(() => {});
      }
    }, 450);
  }

  /** @param {string} path */
  function warmSelection(path) {
    if (!isTauri || !path) return;
    clearTimeout(selectionWarmTimer);
    selectionWarmTimer = setTimeout(() => {
      if (getCurrentMode() !== "cull") return;
      const view = getView();
      const sel = getSel();
      if (view[sel]?.path !== path) return;
      invoke("prefetch_photo", { path }).catch(() => {});
    }, 450);
  }

  return {
    prefetchNeighbours,
    warmSelection,
  };
}

/**
 * Manages parking and releasing decoded frames in disk cache.
 *
 * @param {Object} options
 * @param {boolean} options.isTauri
 * @param {(cmd: string, args?: any) => Promise<any>} options.invoke
 * @param {() => string} options.getCurrentMode
 * @param {() => string | null} options.getPhotoPath
 */
export function createWorkingParker({ isTauri, invoke, getCurrentMode, getPhotoPath }) {
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let workingParkTimer;
  /** @type {string | null} */
  let parkedPath = null;

  /** @param {string} path */
  function scheduleWorkingPark(path) {
    clearTimeout(workingParkTimer);
    if (!isTauri || parkedPath === path) return;
    workingParkTimer = setTimeout(() => {
      if (getPhotoPath() !== path || getCurrentMode() !== "dev") return;
      parkedPath = path;
      invoke("park_working_frame", { path }).catch(() => (parkedPath = null));
    }, 1500);
  }

  function scheduleWorkingRelease() {
    clearTimeout(workingParkTimer);
    if (!isTauri || !parkedPath) return;
    workingParkTimer = setTimeout(() => {
      if (getCurrentMode() === "dev") return;
      parkedPath = null;
      invoke("release_working_frame").catch(() => {});
    }, 5000);
  }

  return {
    scheduleWorkingPark,
    scheduleWorkingRelease,
  };
}

/**
 * Creates the GPU / CPU render pump coordinating preview generation.
 *
 * @param {Object} options
 * @param {any} options.developState
 * @param {(cmd: string, args?: any) => Promise<any>} options.invoke
 * @param {() => Promise<void>} options.tick
 * @param {() => string | null} options.getPhotoPath
 * @param {() => boolean} options.isDockedCrop
 * @param {any} options.library
 * @param {(path: string) => Promise<number>} options.freshPreviewVersion
 * @param {() => void} options.refreshFrames
 * @param {(url: string) => void} options.onLoupeBlobCreated
 * @param {(status: string) => void} options.setStatus
 */
export function createRenderPump({
  developState,
  invoke,
  tick,
  getPhotoPath,
  isDockedCrop,
  library,
  freshPreviewVersion,
  refreshFrames,
  onLoupeBlobCreated,
  setStatus,
}) {
  /**
   * @param {number} px
   * @param {boolean} [live]
   * @returns {Promise<void>}
   */
  function scheduleRender(px, live = false) {
    developState.pendingPx = px;
    developState.pendingLive = live;
    return pump();
  }

  async function pump() {
    const photoPath = getPhotoPath();
    if (developState.inflight || developState.pendingPx === null || !photoPath || !developState.recipe) return;

    developState.inflight = true;
    const px = developState.pendingPx;
    const live = developState.pendingLive;
    developState.pendingPx = null;
    developState.pendingLive = false;
    const path = photoPath;
    const snap = { ...developState.recipe, apply_crop: !isDockedCrop() };
    const t0 = performance.now();

    try {
      if (snap.engine === "rapid") {
        const res = unpackFrame(
          await withTimeout(invoke("develop_preview_rgba", { path, recipe: snap, maxPx: px, live }), RENDER_TIMEOUT_MS),
        );
        if (path === getPhotoPath()) {
          developState.renderMs = Math.round(performance.now() - t0);
          developState.useCanvas = true;
          if (!live || developState.renderAspect === null) {
            developState.renderAspect = res.height ? res.width / res.height : null;
          }
          await tick();
          if (developState.canvasEl) {
            developState.canvasEl.width = res.width;
            developState.canvasEl.height = res.height;
            const ctx = developState.canvasEl.getContext("2d");
            if (ctx) {
              const imgData = new ImageData(/** @type {any} */ (res.rgba), res.width, res.height);
              ctx.putImageData(imgData, 0, 0);
              developState.canvasVersion++;
            }
          }
          developState.imgFailed = false;
          setStatus("");
          const frame = library.frames.find((/** @type {any} */ item) => item.path === path);
          if (frame && px >= PREVIEW_PX) {
            frame.previewVersion = await freshPreviewVersion(frame.path);
            refreshFrames();
          }
        }
      } else {
        const bytes = await withTimeout(invoke("develop_preview", { path, recipe: snap, maxPx: px }), RENDER_TIMEOUT_MS);
        const url = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
        const img = new Image();
        img.src = url;
        try { await img.decode(); } catch (_) {}

        if (path === getPhotoPath()) {
          developState.renderMs = Math.round(performance.now() - t0);
          onLoupeBlobCreated(url);
          developState.useCanvas = false;
          developState.imgFailed = false;
          const frame = library.frames.find((/** @type {any} */ item) => item.path === path);
          if (frame) {
            frame.previewVersion = await freshPreviewVersion(frame.path);
            refreshFrames();
          }
          setStatus("");
        } else {
          URL.revokeObjectURL(url);
        }
      }
    } catch (e) {
      if (path === getPhotoPath()) setStatus(`Error: ${e}`);
    } finally {
      developState.inflight = false;
      if (developState.pendingPx !== null) pump();
    }
  }

  return {
    scheduleRender,
    pump,
  };
}
