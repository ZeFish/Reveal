/**
 * State store for Develop mode.
 *
 * Uses Svelte 5 runes ($state) for fine-grained reactivity across
 * DevelopView, DevelopPanel, Scopes, CurveEditor, and LutStack.
 */

import { session, DEFAULT_PHOTO_SIZE } from "@modules/core";

/** @typedef {Record<string, any>} Recipe */
/** @typedef {{ name: string, label: string, stage?: string }} Profile */
/** @typedef {{ id: string, label: string, control_groups?: any[] }} EngineInfo */
/** @typedef {{ r: Uint32Array, g: Uint32Array, b: Uint32Array, luma: Uint32Array }} HistogramData */

export const MAX_RECIPE_UNDO_STEPS = 50;

/**
 * Creates a deep copy of a recipe snapshot.
 * @param {Recipe | null} r
 * @returns {Recipe | null}
 */
export function snapshotRecipe(r) {
  return r ? JSON.parse(JSON.stringify(r)) : null;
}

export const developState = $state({
  /** @type {Recipe | null} */
  recipe: null,
  /** @type {string | null} */
  developEngine: null,
  /** @type {number | null} */
  renderMs: null,
  /** @type {number | null} */
  renderAspect: null,
  useCanvas: false,
  /** @type {HTMLCanvasElement | null} */
  canvasEl: null,
  canvasVersion: 0,
  imgFailed: false,
  /** @type {HistogramData | null} */
  histogram: null,
  /** @type {any} */
  scopes: null,
  status: "",
  /** @type {Profile[]} */
  films: [],
  /** @type {Profile[]} */
  papers: [],
  /** @type {any[]} */
  luts: [],
  /** @type {EngineInfo[]} */
  engines: [],
  lastEditedKey: "exposure_ev",
  checkLayer: "none",
  lastActiveCheckLayer: "clipping",
  showCaption: false,
  caption: "",
  /** @type {string[]} */
  tags: [],
  /** @type {Recipe | null} */
  developDefaults: null,
  devPublishing: false,
  devPublishStatus: "",
  /** @type {Recipe | null} */
  copiedRecipe: null,
  dockedActiveTab: "dev",
  dockedActiveZone: "all",
  /** @type {'shadows' | 'midtones' | 'highlights' | null} */
  zoneMaskPreview: null,
  developPhotoPercent: typeof window !== "undefined" ? session.photoSize() : DEFAULT_PHOTO_SIZE,
  inflight: false,
  /** @type {number | null} */
  pendingPx: null,
  pendingLive: false,
  /** @type {Recipe[]} */
  recipeUndoStack: [],
  /** @type {Recipe[]} */
  recipeRedoStack: [],
  /** @type {Recipe | null} */
  lastCommittedRecipe: null,
  restoringRecipeHistory: false,
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  saveTimer: undefined,
  recipeSaveBusy: false,
  /** @type {Map<string, any>} */
  recipeSaveQueue: new Map(),

  get showClipping() {
    return this.checkLayer !== "none";
  },
  get canUndo() {
    return this.recipeUndoStack.length > 0;
  },
  get canRedo() {
    return this.recipeRedoStack.length > 0;
  },
  resetHistory() {
    this.recipeUndoStack = [];
    this.recipeRedoStack = [];
    this.lastCommittedRecipe = snapshotRecipe(this.recipe);
  },
});
