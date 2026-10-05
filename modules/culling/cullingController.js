/**
 * Culling Controller.
 *
 * Coordinates grid configuration, star ratings, and vision-assisted culling.
 */

import {
  saveGridPrefs as opSaveGridPrefs,
  toggleLayout as opToggleLayout,
  rate as opRate,
  stopCull as opStopCull,
  triggerAiCull as opTriggerAiCull,
  cullCurrentFolder as opCullCurrentFolder,
} from "./cullingOperations.js";
import { cullingState } from "./cullingState.svelte.js";

/**
 * Creates a bound culling controller instance.
 *
 * @param {{
 *   session?: any,
 *   getView: () => any[],
 *   getFrames?: () => any[],
 *   getCurrentDir: () => string | null,
 *   openDir?: (dir: string, restoreMode?: boolean, restoreScroll?: boolean, keepFilter?: boolean) => Promise<any> | void,
 *   loadStory?: () => Promise<any> | void,
 *   refreshStory?: () => Promise<any> | void,
 *   refreshStoryDirs?: () => Promise<any> | void,
 *   isApplePhotosActive?: () => boolean,
 * }} deps
 */
export function createCullingController(deps) {
  const {
    session,
    getView,
    getFrames = () => [],
    getCurrentDir,
    openDir,
    loadStory = () => {},
    refreshStory = () => {},
    refreshStoryDirs = () => {},
    isApplePhotosActive = () => false,
  } = deps;

  function saveGridPrefs() {
    opSaveGridPrefs(session);
  }

  function toggleLayout() {
    opToggleLayout({ totalFrames: getFrames().length, session });
  }

  /** @param {number} n */
  function rate(n) {
    return opRate(getView(), n);
  }

  /** @param {number} n */
  function setMinRating(n) {
    cullingState.minRating = n;
    const curDir = getCurrentDir();
    if (curDir && openDir) {
      openDir(curDir, true, false, true);
    }
  }

  /**
   * @param {string} dir
   * @param {string[]} paths
   */
  function triggerAiCull(dir, paths) {
    return opTriggerAiCull(dir, paths, async () => {
      if (dir === getCurrentDir()) {
        await refreshStory();
        refreshStoryDirs();
      }
    });
  }

  /** @param {string} [dir] */
  function cullCurrentFolder(dir) {
    return opCullCurrentFolder({
      dir: dir ?? getCurrentDir(),
      view: getView(),
      isApplePhotos: isApplePhotosActive(),
      onStoryRefreshed: async () => {
        await loadStory();
        refreshStoryDirs();
      },
    });
  }

  /** @param {string} text */
  function setTextFilter(text) {
    cullingState.textFilter = text;
  }

  /** @param {"all" | "picks" | "rejected" | "unflagged"} pick */
  function setPickFilter(pick) {
    cullingState.pickFilter = pick;
  }

  function resetFilters() {
    cullingState.minRating = 0;
    cullingState.textFilter = "";
    cullingState.pickFilter = "all";
  }

  return {
    saveGridPrefs,
    toggleLayout,
    rate,
    setMinRating,
    setTextFilter,
    setPickFilter,
    resetFilters,
    triggerAiCull,
    cullCurrentFolder,
    stopCull: opStopCull,
  };
}
