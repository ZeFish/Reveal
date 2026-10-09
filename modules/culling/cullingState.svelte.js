/**
 * Culling & Grid Layout state using Svelte 5 runes.
 */

export const MASONRY_LIMIT = 500;

export const cullingState = $state({
  cols: 4,
  marginScale: 1,
  cellAspect: 1.5,
  fillCells: true,
  sortDesc: true,
  /** @type {"uniform" | "masonry"} */
  layout: "uniform",
  minRating: 0,
  textFilter: "",
  /** @type {"all" | "picks" | "rejected" | "unflagged"} */
  pickFilter: "all",
  /** @type {string | null} */
  activeVirtualCollectionId: null,
  /** @type {string | null} */
  cullTaskId: null,
  aiCullMarkStory: false,
  aiCullExportDesktop: false,
  aiCullTarget: 24,
});

export function resetFilters() {
  cullingState.minRating = 0;
  cullingState.textFilter = "";
  cullingState.pickFilter = "all";
}

/**
 * Initialize culling state from saved session & user preferences.
 * @param {Object} params
 * @param {any} [params.gridPrefs]
 * @param {"uniform" | "masonry" | null} [params.gridLayout]
 * @param {any} [params.preferences]
 */
export function initCullingState({ gridPrefs, gridLayout, preferences } = {}) {
  if (gridPrefs) {
    const savedCols = Math.trunc(Number(gridPrefs.cols));
    if (Number.isFinite(savedCols) && savedCols >= 1 && savedCols <= 12) {
      cullingState.cols = savedCols;
    }
    const savedMargin = Number(gridPrefs.marginScale);
    if (Number.isFinite(savedMargin)) {
      cullingState.marginScale = Math.min(6, Math.max(0.25, savedMargin));
    }
    const savedAspect = Number(gridPrefs.cellAspect);
    if (Number.isFinite(savedAspect)) {
      cullingState.cellAspect = Math.min(3, Math.max(0.5, savedAspect));
    }
    if (typeof gridPrefs.fillCells === "boolean") cullingState.fillCells = gridPrefs.fillCells;
    if (typeof gridPrefs.sortDesc === "boolean") cullingState.sortDesc = gridPrefs.sortDesc;
  }
  if (gridLayout) {
    cullingState.layout = gridLayout;
  }
  if (preferences) {
    cullingState.aiCullMarkStory = !!preferences.ai_cull_mark_story;
    cullingState.aiCullExportDesktop = !!preferences.ai_cull_export_desktop;
    cullingState.aiCullTarget = Number(preferences.ai_cull_target) || 24;
  }
}
