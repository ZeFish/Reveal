/**
 * Public API for the Develop module.
 *
 * Colocalizes develop state, operations, and UI components (DevelopView, DevelopPanel, Scopes, CurveEditor, LutStack).
 */

export {
  developState,
  snapshotRecipe,
  MAX_RECIPE_UNDO_STEPS,
} from "./developState.svelte.js";

export {
  RENDER_TIMEOUT_MS,
  PREVIEW_PX,
  unpackFrame,
  withTimeout,
  scheduleRender,
  recordRecipeCommit,
  undoRecipeEdit,
  redoRecipeEdit,
  queueRecipeSave,
  handleRecipeEdited,
  setDevNum,
  resetOne,
  lutsKey,
  oldLutsKey,
  ensureLutMigration,
  addLutLayer,
  removeLutLayer,
  updateLutOpacity,
  setLutFile,
  applyEngineChange,
  applyResetRecipe,
  toggleCheckLayer,
  copySettings,
  applyRecipeToFrames,
  showAsShot,
  captionEdited,
  tagsEdited,
} from "./developOperations.js";

export {
  createPrefetcher,
  createWorkingParker,
  createRenderPump,
} from "./developRunner.js";

export {
  createDevelopController,
} from "./developController.js";

export {
  createPhotoLoader,
  previewUrl,
  openingUrl,
} from "./photoLoader.js";

export { createDevPanelMirror } from "./devPanelMirror.svelte.js";

export {
  createScopeAnalyzer,
  renderCheckLayer,
  WF_COLS,
  WF_LEVELS,
  VEC_SIZE,
} from "./developAnalysis.js";

export {
  registerEngine,
  unregisterEngine,
  getEngineComponent,
  listRegisteredEngines,
} from "./engines/engineRegistry.js";

export { default as DevelopView } from "./DevelopView.svelte";
export { default as DevelopPanel } from "./DevelopPanel.svelte";
export { default as DevelopWorkspace } from "./DevelopWorkspace.svelte";
export { default as EngineRunner } from "./EngineRunner.svelte";

export { default as CaptionOverlay } from "./CaptionOverlay.svelte";
export { default as CheckLayerOverlay } from "./CheckLayerOverlay.svelte";
export { default as CropOverlay } from "./CropOverlay.svelte";
export { default as DevelopLoupe } from "./DevelopLoupe.svelte";
export { default as Scopes } from "./Scopes.svelte";
export { default as CurveEditor } from "./CurveEditor.svelte";
export { default as LutStack } from "./LutStack.svelte";
