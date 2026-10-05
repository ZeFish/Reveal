/**
 * Export operations controller.
 *
 * Coordinates grid, selection, single photo, and Obsidian daily note exports.
 */

import {
  exportGrid as opExportGrid,
  exportSelection as opExportSelection,
  exportCurrent as opExportCurrent,
  exportToDailyNote as opExportToDailyNote,
  exportSelectionToDailyNote as opExportSelectionToDailyNote,
  exportToDestination as opExportToDestination,
} from "./exportOperations.js";

/**
 * Creates a bound export operations controller.
 *
 * @param {{
 *   getView: () => any[],
 *   getSel: () => number,
 *   getSelection: () => { paths: Set<string> },
 *   getPhotoPath: () => string | null,
 *   getRecipe: () => any,
 *   getPicked: () => string | null | undefined,
 *   exportState: { folder: string, edge: number, border: boolean },
 *   selectedFrames: (view: any[]) => any[],
 * }} deps
 */
export function createExportController(deps) {
  const {
    getView,
    getSel,
    getSelection,
    getPhotoPath,
    getRecipe,
    getPicked,
    exportState,
    selectedFrames,
  } = deps;

  function exportGrid() {
    return opExportGrid({
      view: getView(),
      destDir: exportState.folder,
      longEdge: exportState.edge,
      borderFrac: exportState.border ? 0.04 : 0,
    });
  }

  function exportSelection() {
    return opExportSelection({
      targets: selectedFrames(getView()),
      destDir: exportState.folder,
      longEdge: exportState.edge,
      borderFrac: exportState.border ? 0.04 : 0,
    });
  }

  /** @param {string} [destDir] override the configured export folder */
  function exportCurrent(destDir = exportState.folder) {
    return opExportCurrent({
      photoPath: getPhotoPath(),
      recipe: getRecipe(),
      picked: getPicked() ?? "",
      destDir,
      longEdge: exportState.edge,
      borderFrac: exportState.border ? 0.04 : 0,
    });
  }

  /** @param {string} [targetPath] @param {any} [customRecipe] */
  function exportToDailyNote(targetPath, customRecipe) {
    const view = getView();
    const sel = getSel();
    const photoPath = getPhotoPath();
    const target = targetPath || photoPath || view[sel]?.path;
    if (!target) return;
    const rec = customRecipe || (target === photoPath ? getRecipe() : null);
    return opExportToDailyNote({
      target,
      recipe: rec,
      longEdge: exportState.edge,
      borderFrac: exportState.border ? 0.04 : 0,
    });
  }

  /** @param {string} [clickedPath] */
  function exportSelectionToDailyNote(clickedPath) {
    const view = getView();
    const sel = getSel();
    const selection = getSelection();
    const targets = selection.paths.size > 0
      ? view.filter((f) => selection.paths.has(f.path)).map((f) => f.path)
      : (clickedPath ? [clickedPath] : (view[sel] ? [view[sel].path] : []));
    return opExportSelectionToDailyNote({
      targets,
      longEdge: exportState.edge,
      borderFrac: exportState.border ? 0.04 : 0,
      onSingleExport: (t) => exportToDailyNote(t),
    });
  }

  /**
   * Export to an arbitrary Destination model instance.
   * @param {import('@modules/core').Destination} destination
   * @param {any[]} [customTargets]
   */
  function exportTo(destination, customTargets) {
    const targets = customTargets || selectedFrames(getView());
    const photoPath = getPhotoPath();
    const finalTargets = targets.length > 0 ? targets : (photoPath ? [{ path: photoPath }] : []);
    return opExportToDestination({
      destination,
      targets: finalTargets,
      recipe: getRecipe(),
      picked: getPicked() ?? "",
    });
  }

  return {
    exportGrid,
    exportSelection,
    exportCurrent,
    exportToDailyNote,
    exportSelectionToDailyNote,
    exportTo,
  };
}
