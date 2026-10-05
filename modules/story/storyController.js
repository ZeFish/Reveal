/**
 * Story operations controller.
 *
 * Encapsulates story note persistence, grid prose inserts, story inclusion toggles,
 * and publishing/exporting workflow.
 */

import {
  loadStory as opLoadStory,
  saveStoryContent as opSaveStoryContent,
  saveGridProse as opSaveGridProse,
  refreshStory as opRefreshStory,
  toggleStoryWithPath as opToggleStoryWithPath,
  publishStory as opPublishStory,
  exportLocalStory as opExportLocalStory,
} from "./storyOperations.js";

/**
 * Creates a bound story operations controller.
 *
 * @param {{
 *   getDir: () => string | null,
 *   getView: () => any[],
 *   getSel: () => number,
 *   getCols: () => number,
 *   getSignedIn: () => boolean | undefined,
 *   getExportState: () => { edge: number, border: boolean },
 *   onRefreshed?: () => Promise<void> | void,
 * }} deps
 */
export function createStoryController(deps) {
  const {
    getDir,
    getView,
    getSel,
    getCols,
    getSignedIn,
    getExportState,
    onRefreshed,
  } = deps;

  async function loadStory() {
    await opLoadStory(getDir());
  }

  /** @param {string} content */
  async function saveStoryContent(content) {
    await opSaveStoryContent(getDir(), content, onRefreshed);
  }

  /**
   * @param {number} row
   * @param {string} text
   * @param {string} [blockId]
   */
  async function saveGridProse(row, text, blockId) {
    await opSaveGridProse({
      dir: getDir(),
      row,
      text,
      blockId,
      view: getView(),
      cols: getCols(),
      onRefreshed,
    });
  }

  async function refreshStory() {
    await opRefreshStory(getDir());
  }

  /** @param {string} path */
  async function toggleStoryWithPath(path) {
    await opToggleStoryWithPath(path, getDir(), onRefreshed);
  }

  async function toggleStory() {
    const view = getView();
    const sel = getSel();
    const frame = view[sel];
    if (frame) await toggleStoryWithPath(frame.path);
  }

  async function publishStory() {
    await opPublishStory({
      dir: getDir(),
      signedIn: getSignedIn(),
    });
  }

  async function exportLocalStory() {
    const exportState = getExportState();
    await opExportLocalStory({
      dir: getDir(),
      longEdge: exportState.edge,
      borderFrac: exportState.border ? 0.04 : 0,
    });
  }

  return {
    loadStory,
    saveStoryContent,
    saveGridProse,
    refreshStory,
    toggleStoryWithPath,
    toggleStory,
    publishStory,
    exportLocalStory,
  };
}
