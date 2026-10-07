/**
 * Develop Controller.
 *
 * Coordinates docked develop panel events, recipe editing, LUT layer
 * mutations, and engine transitions with the active viewer state.
 */

import {
  handleRecipeEdited,
  undoRecipeEdit as opUndoRecipeEdit,
  redoRecipeEdit as opRedoRecipeEdit,
  showAsShot as opShowAsShot,
  setDevNum as opSetDevNum,
  resetOne as opResetOne,
  addLutLayer as opAddLutLayer,
  removeLutLayer as opRemoveLutLayer,
  updateLutOpacity as opUpdateLutOpacity,
  setLutFile as opSetLutFile,
  applyEngineChange as opApplyEngineChange,
  applyResetRecipe as opApplyResetRecipe,
  toggleCheckLayer as opToggleCheckLayer,
  captionEdited as opCaptionEdited,
  tagsEdited as opTagsEdited,
  copySettings as opCopySettings,
  applyRecipeToFrames as opApplyRecipeToFrames,
} from "./developOperations.js";

/**
 * Creates a bound develop controller instance.
 *
 * @param {{
 *   state: any,
 *   getPhotoPath: () => string | null,
 *   getCurrentMode: () => "dev" | "cull",
 *   liveRenderPx: (live: boolean) => number,
 *   scheduleRender: (px: number) => Promise<any> | void,
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   emit?: (event: string, payload?: any) => Promise<any> | void,
 *   hold?: (msg: string) => void,
 *   notify?: (msg: string, ms?: number) => void,
 *   isTauri?: boolean,
 *   library?: any,
 *   freshPreviewVersion?: (path: string) => Promise<number>,
 *   refreshFrames?: () => void,
 *   previewUrl?: (path: string, version?: number, asShot?: boolean) => string,
 *   getImgUrl?: () => string | null,
 *   setImgUrl?: (url: string | null) => void,
 *   sendDevStateToPanel?: () => void,
 *   layouts?: any,
 *   saveLayouts?: () => void,
 *   saveExportPrefs?: () => void,
 *   getSourceFrame?: () => any,
 *   getSelectedFrames?: () => any[],
 *   activity?: any,
 *   startActivity?: (type: string, title: string, total: number) => string,
 *   updateActivity?: (id: string, update: any) => void,
 *   releaseActive?: (id: string) => void,
 *   setProgress?: (p: any) => void,
 *   patchProgress?: (p: any) => void,
 *   advanceProgress?: () => number,
 *   previewPx?: number,
 *   onUpdateLoupeUrl?: (bytes: any) => void,
 * }} deps
 */
export function createDevelopController(deps) {
  const {
    state,
    getPhotoPath,
    getCurrentMode,
    liveRenderPx,
    scheduleRender,
    invoke,
    emit,
    hold = () => {},
    isTauri = false,
    library,
    freshPreviewVersion,
    refreshFrames,
    previewUrl,
    getImgUrl = () => null,
    setImgUrl = () => {},
    sendDevStateToPanel = () => {},
    layouts,
    saveLayouts = () => {},
    saveExportPrefs = () => {},
    notify = () => {},
    getSourceFrame,
    getSelectedFrames,
    activity,
    startActivity,
    updateActivity,
    releaseActive,
    setProgress,
    patchProgress,
    advanceProgress,
    previewPx = 2048,
    onUpdateLoupeUrl,
  } = deps;

  const captionTimerRef = { current: undefined };

  /** @param {boolean} [live] */
  function edited(live = false) {
    handleRecipeEdited(state, live, {
      photoPath: getPhotoPath(),
      liveRenderPx,
      scheduleRender,
      invoke,
      hold,
    });
  }

  /**
   * @param {boolean} [transient]
   * @param {string} [key]
   */
  function dockedEdited(transient = false, key = undefined) {
    if (key) state.lastEditedKey = key;
    edited(transient);
  }

  async function clearDevelopment() {
    const path = getPhotoPath();
    if (!path) return;
    // Taking the develop settings off the photo's folder is a NAS write, owed to the photo and
    // paid by the backend once the choice of engine has stood still (photo_writes.rs). A failure
    // to queue it is not worth stopping the switch for.
    invoke("queue_clear_development", { path }).catch(() => {});
    // The camera's own picture, at once — no waiting on the disk.
    await opShowAsShot({
      state,
      photoPath: path,
      previewUrl,
      onSetLoupeUrl: (next) => {
        const cur = getImgUrl();
        if (cur?.startsWith("blob:")) URL.revokeObjectURL(cur);
        setImgUrl(next);
      },
    });
  }

  /** @param {string | null} engineId */
  function applyEngineChange(engineId) {
    // An engine chosen again within the delay: there is nothing left to clear.
    const path = getPhotoPath();
    if (engineId !== null && path) invoke("cancel_photo_writes", { path }).catch(() => {});
    opApplyEngineChange(state, engineId, clearDevelopment, (live) => edited(live));
  }

  /** @param {string | null} [mode] */
  function toggleCheckLayer(mode = null) {
    opToggleCheckLayer(state, mode, (layer) => {
      sendDevStateToPanel();
      if (isTauri && emit) emit("dev-panel-set-check-layer", { checkLayer: layer });
    });
  }

  async function copySettings() {
    const source = getSourceFrame?.();
    if (!source) return;
    await opCopySettings(state, source, {
      photoPath: getPhotoPath(),
      invoke,
      notify,
    });
  }

  /**
   * @param {any} recipeToApply
   * @param {any[]} targetFrames
   */
  async function applyRecipeToFrames(recipeToApply, targetFrames) {
    await opApplyRecipeToFrames({
      state,
      recipeToApply,
      targetFrames,
      photoPath: getPhotoPath(),
      activity,
      startActivity,
      updateActivity,
      releaseActive,
      setProgress,
      patchProgress,
      advanceProgress,
      invoke,
      freshPreviewVersion,
      refreshFrames,
      scheduleRender,
      sendDevStateToPanel,
      notify,
      PREVIEW_PX: previewPx,
      onUpdateLoupeUrl,
    });
  }

  async function pasteSettings() {
    if (!state.copiedRecipe) return;
    const targets = getSelectedFrames ? getSelectedFrames() : [];
    await applyRecipeToFrames(state.copiedRecipe, targets);
  }

  return {
    edited,
    dockedEdited,
    captionEdited: () => opCaptionEdited(state, getPhotoPath(), invoke, hold, captionTimerRef),
    tagsEdited: () => opTagsEdited(state, getPhotoPath(), invoke, hold),
    undoRecipeEdit: () => opUndoRecipeEdit(state, getCurrentMode(), (live) => edited(live)),
    redoRecipeEdit: () => opRedoRecipeEdit(state, getCurrentMode(), (live) => edited(live)),
    clearDevelopment,
    setDevNum: (
      /** @type {string} */ key,
      /** @type {number | string} */ v,
      /** @type {boolean} */ transient = false,
      /** @type {number | undefined} */ index = undefined,
    ) => opSetDevNum(state, key, v, transient, index, dockedEdited),
    resetOne: (/** @type {string} */ key, /** @type {number | undefined} */ index = undefined) =>
      opResetOne(state, key, index, dockedEdited),
    addLutLayer: (/** @type {string} */ stage) => opAddLutLayer(state, stage, dockedEdited),
    removeLutLayer: (/** @type {string} */ stage, /** @type {number} */ index) => opRemoveLutLayer(state, stage, index, dockedEdited),
    updateLutOpacity: (/** @type {string} */ stage, /** @type {number} */ index, /** @type {number | string} */ value) =>
      opUpdateLutOpacity(state, stage, index, value, dockedEdited),
    setLutFile: (/** @type {string} */ stage, /** @type {number} */ index, /** @type {string} */ name) =>
      opSetLutFile(state, stage, index, name, dockedEdited),
    // Draw the photo again as it now is (the Crop tab shows the whole frame, every
    // other tab the cropped one, so switching between them changes the picture).
    rerender: () => scheduleRender(previewPx),
    applyEngineChange,
    dockedEngineChanged: (/** @type {string} */ value) => applyEngineChange(value === "none" ? null : value),
    applyResetRecipe: () => opApplyResetRecipe(state, invoke, () => edited()),
    toggleCheckLayer,
    dockedToggleClipping: (/** @type {string | null} */ mode = null) => toggleCheckLayer(mode),
    dockedToggleCaptionOverlay: () => {
      state.showCaption = !state.showCaption;
    },
    dockedHidePanel: () => {
      if (layouts?.dev) {
        layouts.dev.devPanel = false;
        saveLayouts();
      }
    },
    dockedExportSettingsChanged: () => {
      saveExportPrefs();
    },
    copySettings,
    applyRecipeToFrames,
    pasteSettings,
  };
}
