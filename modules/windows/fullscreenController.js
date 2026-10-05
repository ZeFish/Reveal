/**
 * Fullscreen controller coordinating borderless fullscreen presentation.
 */

import {
  fullscreenState,
  prepareFullscreenFrame as opPrepareFullscreenFrame,
  enterFullscreen as opEnterFullscreen,
  exitFullscreen as opExitFullscreen,
} from "./fullscreenState.svelte.js";

/**
 * Creates a bound fullscreen controller.
 *
 * @param {{
 *   getLibraryFrames: () => any[],
 *   getCurrentMode: () => string,
 *   getPhotoPath: () => string | null,
 *   getImgUrl: () => string | null,
 *   previewUrl: (path: string, version: number) => string,
 *   getView: () => any[],
 *   getSel: () => number,
 *   getLayouts: () => any,
 *   onSyncDevPanel?: () => Promise<void> | void,
 * }} deps
 */
export function createFullscreenController(deps) {
  const {
    getLibraryFrames,
    getCurrentMode,
    getPhotoPath,
    getImgUrl,
    previewUrl,
    getView,
    getSel,
    getLayouts,
    onSyncDevPanel,
  } = deps;

  /** @param {string} path */
  function prepareFullscreenFrame(path) {
    const frames = getLibraryFrames ? getLibraryFrames() : [];
    const frame = frames.find((item) => item.path === path);
    return opPrepareFullscreenFrame(path, {
      frame,
      currentMode: getCurrentMode ? getCurrentMode() : "cull",
      photoPath: getPhotoPath ? getPhotoPath() : null,
      imgUrl: getImgUrl ? getImgUrl() : null,
      previewUrl,
      getCurrentFramePath: () => {
        const view = getView ? getView() : [];
        const sel = getSel ? getSel() : 0;
        return view[sel]?.path;
      },
    });
  }

  function enterFullscreen() {
    const view = getView ? getView() : [];
    const sel = getSel ? getSel() : 0;
    const frame = view[sel];
    if (!frame) return;
    return opEnterFullscreen({
      frame,
      prepareFrame: (p) => prepareFullscreenFrame(p),
    });
  }

  function exitFullscreen() {
    return opExitFullscreen({
      imgUrl: getImgUrl ? getImgUrl() : null,
      currentMode: getCurrentMode ? getCurrentMode() : "cull",
      layouts: getLayouts ? getLayouts() : null,
      onSyncDevPanel,
    });
  }

  function toggleFullscreen() {
    if (fullscreenState.active) return exitFullscreen();
    return enterFullscreen();
  }

  return {
    prepareFullscreenFrame,
    enterFullscreen,
    exitFullscreen,
    toggleFullscreen,
  };
}
