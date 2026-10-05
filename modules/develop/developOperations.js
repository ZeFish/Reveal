/**
 * Operations for the Develop mode: frame parsing, recipe undo/redo,
 * LUT management, debounced saving, batch recipe application, and rendering pipeline.
 */

import { snapshotRecipe, MAX_RECIPE_UNDO_STEPS } from "./developState.svelte.js";

/** @typedef {Record<string, any>} Recipe */
/** @typedef {{ path: string, name: string, previewVersion?: number }} Frame */
/**
 * @typedef {Object} RgbaPreview
 * @property {number} width
 * @property {number} height
 * @property {number} renderMs
 * @property {Uint8ClampedArray} rgba
 */

export const RENDER_TIMEOUT_MS = 60_000;
export const PREVIEW_PX = 2048;

/**
 * Reads a frame off the raw IPC channel: four u32 of header, then the pixels.
 * @param {ArrayBuffer | ArrayBufferView | number[]} buf
 * @returns {RgbaPreview}
 */
export function unpackFrame(buf) {
  if (!buf) throw new Error("empty frame buffer");
  let ab;
  if (buf instanceof ArrayBuffer) {
    ab = buf;
  } else if (ArrayBuffer.isView(buf)) {
    ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
  } else if (Array.isArray(buf)) {
    ab = new Uint8Array(buf).buffer;
  } else {
    throw new Error(`unexpected frame type: ${typeof buf}`);
  }
  if (ab.byteLength < 16) throw new Error("frame buffer too small for header");
  const arrayBuffer = /** @type {ArrayBuffer} */ (ab);
  const head = new DataView(arrayBuffer, 0, 16);
  const width = head.getUint32(0, true);
  const height = head.getUint32(4, true);
  const renderMs = head.getUint32(8, true);
  const expectedBytes = width * height * 4;
  const availableBytes = Math.max(0, arrayBuffer.byteLength - 16);
  const byteCount = Math.min(expectedBytes, availableBytes);
  return {
    width,
    height,
    renderMs,
    rgba: new Uint8ClampedArray(arrayBuffer, 16, byteCount),
  };
}

/**
 * Fails a promise that has not answered within `ms`.
 * @template T
 * @param {Promise<T>} promise
 * @param {number} ms
 * @param {string} [customError]
 * @returns {Promise<T>}
 */
export function withTimeout(promise, ms, customError) {
  /** @type {ReturnType<typeof setTimeout>} */
  let timer;
  const limit = new Promise((_, reject) => {
    timer = setTimeout(
      () =>
        reject(
          new Error(
            customError ||
              "the drive is not answering. Check that the NAS is reachable, then try again.",
          ),
        ),
      ms,
    );
  });
  return /** @type {Promise<T>} */ (
    Promise.race([promise, limit]).finally(() => clearTimeout(timer))
  );
}

/**
 * Queues a render request.
 * @param {any} state
 * @param {number} px
 * @param {boolean} [live]
 * @param {() => void} [pumpFn]
 */
export function scheduleRender(state, px, live = false, pumpFn = undefined) {
  state.pendingPx = px;
  state.pendingLive = live;
  if (pumpFn) pumpFn();
}

/**
 * Records a settled recipe state to the undo history stack.
 * @param {any} state
 * @param {number} [maxUndoSteps]
 */
export function recordRecipeCommit(state, maxUndoSteps = MAX_RECIPE_UNDO_STEPS) {
  if (state.lastCommittedRecipe) {
    state.recipeUndoStack.push(state.lastCommittedRecipe);
    if (state.recipeUndoStack.length > maxUndoSteps) {
      state.recipeUndoStack.shift();
    }
  }
  state.recipeRedoStack = [];
  state.lastCommittedRecipe = snapshotRecipe(state.recipe);
}

/**
 * Undoes the last recipe edit.
 * @param {any} state
 * @param {"dev" | "cull"} currentMode
 * @param {(live: boolean) => void} [onEdited]
 */
export function undoRecipeEdit(state, currentMode, onEdited) {
  if (!state.recipeUndoStack.length || !state.recipe || currentMode !== "dev") return;
  const previous = state.recipeUndoStack.pop();
  state.recipeRedoStack.push(snapshotRecipe(state.recipe));
  state.restoringRecipeHistory = true;
  state.recipe = previous;
  state.developEngine =
    previous.engine === "rapid"
      ? "rapid"
      : previous.engine
        ? "spektra"
        : state.developEngine;
  state.lastCommittedRecipe = snapshotRecipe(state.recipe);
  if (onEdited) onEdited(false);
  state.restoringRecipeHistory = false;
}

/**
 * Redoes the last undone recipe edit.
 * @param {any} state
 * @param {"dev" | "cull"} currentMode
 * @param {(live: boolean) => void} [onEdited]
 */
export function redoRecipeEdit(state, currentMode, onEdited) {
  if (!state.recipeRedoStack.length || !state.recipe || currentMode !== "dev") return;
  const next = state.recipeRedoStack.pop();
  state.recipeUndoStack.push(snapshotRecipe(state.recipe));
  state.restoringRecipeHistory = true;
  state.recipe = next;
  state.developEngine =
    next.engine === "rapid"
      ? "rapid"
      : next.engine
        ? "spektra"
        : state.developEngine;
  state.lastCommittedRecipe = snapshotRecipe(state.recipe);
  if (onEdited) onEdited(false);
  state.restoringRecipeHistory = false;
}

/**
 * Debounced queue for saving recipe sidecars to disk.
 * @param {string} path
 * @param {any} snapshot
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   hold?: (msg: string) => void,
 *   state: any
 * }} context
 */
export function saveRecipeSoon(path, snapshot, { invoke, hold, state }) {
  if (state.recipeSaveBusy) {
    state.recipeSaveQueue.set(path, snapshot);
    return;
  }
  state.recipeSaveBusy = true;
  invoke("save_recipe", { path, recipe: snapshot })
    .catch((error) => {
      if (hold) hold(`Could not save development settings: ${error}`);
    })
    .finally(() => {
      state.recipeSaveBusy = false;
      const next = state.recipeSaveQueue.entries().next();
      if (!next.done) {
        state.recipeSaveQueue.delete(next.value[0]);
        saveRecipeSoon(next.value[0], next.value[1], { invoke, hold, state });
      }
    });
}

/**
 * Main edit trigger called when controls change.
 * @param {any} state
 * @param {boolean} live
 * @param {{
 *   photoPath: string | null,
 *   liveRenderPx: (live: boolean) => number,
 *   scheduleRender: (px: number, live?: boolean) => void,
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   hold?: (msg: string) => void,
 *   debounceMs?: number,
 *   maxUndoSteps?: number,
 * }} options
 */
export function handleRecipeEdited(
  state,
  live = false,
  {
    photoPath,
    liveRenderPx,
    scheduleRender: scheduleRenderFn,
    invoke,
    hold,
    debounceMs = 300,
    maxUndoSteps = MAX_RECIPE_UNDO_STEPS,
  },
) {
  if (!state.developEngine || state.developEngine === "none") {
    state.developEngine = "spektra";
  }
  if (state.recipe) {
    state.recipe.engine = state.developEngine;
  }
  if (!live && !state.restoringRecipeHistory && state.recipe) {
    recordRecipeCommit(state, maxUndoSteps);
  }
  if (scheduleRenderFn && liveRenderPx) {
    scheduleRenderFn(liveRenderPx(live), live);
  }
  clearTimeout(state.saveTimer);
  const path = photoPath;
  const snapshot = state.recipe ? { ...state.recipe } : null;
  state.saveTimer = setTimeout(() => {
    if (path && snapshot && typeof invoke === "function") {
      saveRecipeSoon(path, snapshot, { invoke, hold, state });
    }
  }, debounceMs);
}

/**
 * Sets a recipe property directly, with support for indexed arrays.
 * @param {any} state
 * @param {string} key
 * @param {number | string} v
 * @param {boolean} [transient]
 * @param {number} [index]
 * @param {(transient?: boolean, key?: string) => void} [onEdited]
 */
export function setDevNum(state, key, v, transient = false, index = undefined, onEdited = undefined) {
  if (!state.recipe) return;
  const r = state.recipe;
  if (index != null) {
    if (!Array.isArray(r[key])) r[key] = [];
    r[key][index] = Number(v);
  } else {
    r[key] = Number(v);
  }
  state.lastEditedKey = key;
  if (onEdited) onEdited(transient, key);
}

/**
 * Resets a single recipe key to its default value.
 * @param {any} state
 * @param {string} key
 * @param {number} [index]
 * @param {(transient?: boolean, key?: string) => void} [onEdited]
 */
export function resetOne(state, key, index = undefined, onEdited = undefined) {
  const d = state.developDefaults;
  if (!d || !state.recipe || !(key in d)) return;
  if (index != null) {
    setDevNum(state, key, d[key]?.[index] ?? 0, false, index, onEdited);
  } else {
    setDevNum(state, key, d[key], false, undefined, onEdited);
  }
}

/** @param {string} stage */
export const lutsKey = (stage) => (stage === "pre" ? "rapid_pre_luts" : "rapid_post_luts");
/** @param {string} stage */
export const oldLutsKey = (stage) => (stage === "pre" ? "pre_luts" : "post_luts");

/**
 * Migrates legacy LUT arrays in recipe if present.
 * @param {any} state
 * @param {string} stage
 * @param {() => void} [onEdited]
 */
export function ensureLutMigration(state, stage, onEdited = undefined) {
  const r = state.recipe;
  if (!r || state.developEngine !== "rapid") return;
  const key = lutsKey(stage);
  const oldKey = oldLutsKey(stage);
  if (r[oldKey]?.length > 0) {
    r[key] = [...(r[key] ?? []), ...r[oldKey]];
    r[oldKey] = [];
    if (onEdited) onEdited();
  }
}

/**
 * Adds a new LUT layer to the recipe.
 * @param {any} state
 * @param {string} stage
 * @param {() => void} [onEdited]
 */
export function addLutLayer(state, stage, onEdited = undefined) {
  if (!state.recipe) return;
  const r = state.recipe;
  ensureLutMigration(state, stage, onEdited);
  const key = lutsKey(stage);
  const first = typeof state.luts[0] === "string" ? state.luts[0] : (state.luts[0]?.name ?? "");
  r[key] = [...(r[key] ?? []), { name: first, opacity: 1 }];
  if (onEdited) onEdited();
}

/**
 * Removes a LUT layer by index.
 * @param {any} state
 * @param {string} stage
 * @param {number} index
 * @param {() => void} [onEdited]
 */
export function removeLutLayer(state, stage, index, onEdited = undefined) {
  if (!state.recipe) return;
  const r = state.recipe;
  ensureLutMigration(state, stage, onEdited);
  const key = lutsKey(stage);
  r[key] = (/** @type {any[]} */ (r[key] ?? [])).filter((/** @type {any} */ _, /** @type {number} */ i) => i !== index);
  if (onEdited) onEdited();
}

/**
 * Updates opacity of a LUT layer.
 * @param {any} state
 * @param {string} stage
 * @param {number} index
 * @param {number | string} value
 * @param {(transient?: boolean) => void} [onEdited]
 */
export function updateLutOpacity(state, stage, index, value, onEdited = undefined) {
  if (!state.recipe) return;
  const r = state.recipe;
  ensureLutMigration(state, stage, onEdited);
  const key = lutsKey(stage);
  r[key] = (/** @type {any[]} */ (r[key] ?? [])).map((/** @type {any} */ l, /** @type {number} */ i) =>
    i === index ? { ...l, opacity: Number(value) } : l,
  );
  if (onEdited) onEdited(true);
}

/**
 * Sets file name for a LUT layer.
 * @param {any} state
 * @param {string} stage
 * @param {number} index
 * @param {string} name
 * @param {() => void} [onEdited]
 */
export function setLutFile(state, stage, index, name, onEdited = undefined) {
  if (!state.recipe) return;
  const r = state.recipe;
  ensureLutMigration(state, stage, onEdited);
  const key = lutsKey(stage);
  r[key] = (/** @type {any[]} */ (r[key] ?? [])).map((/** @type {any} */ l, /** @type {number} */ i) =>
    i === index ? { ...l, name } : l,
  );
  if (onEdited) onEdited();
}

/**
 * Handles engine change or reset to none.
 * @param {any} state
 * @param {string | null} engineId
 * @param {() => void} [onClearDevelopment]
 * @param {(live: boolean) => void} [onEdited]
 */
export function applyEngineChange(state, engineId, onClearDevelopment = undefined, onEdited = undefined) {
  if (engineId === null) {
    if (onClearDevelopment) onClearDevelopment();
  } else {
    state.developEngine = engineId;
    if (state.recipe) {
      state.recipe.engine = state.developEngine === "rapid" ? "rapid" : "spektra";
    }
    if (onEdited) onEdited(false);
  }
}

/**
 * Resets the current recipe to default values.
 * @param {any} state
 * @param {(cmd: string, args?: any) => Promise<any>} invoke
 * @param {() => void} [onEdited]
 */
export async function applyResetRecipe(state, invoke, onEdited = undefined) {
  if (state.recipe) {
    state.recipe = await invoke("default_recipe");
    if (onEdited) onEdited();
  }
}

/**
 * Toggles or sets the check layer overlay mode.
 * @param {any} state
 * @param {string | null} [mode]
 * @param {(checkLayer: string) => void} [onChanged]
 */
export function toggleCheckLayer(state, mode = null, onChanged = undefined) {
  if (mode) {
    state.checkLayer = state.checkLayer === mode ? "none" : mode;
    if (state.checkLayer !== "none") state.lastActiveCheckLayer = state.checkLayer;
  } else {
    if (state.checkLayer !== "none") {
      state.checkLayer = "none";
    } else {
      state.checkLayer = state.lastActiveCheckLayer || "clipping";
    }
  }
  if (onChanged) onChanged(state.checkLayer);
}

/**
 * Copies develop settings from the selected frame (excluding crop).
 * @param {any} state
 * @param {{ path: string, name: string }} source
 * @param {{
 *   photoPath: string | null,
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void
 * }} context
 */
export async function copySettings(state, source, { photoPath, invoke, notify }) {
  if (!source) return;
  /** @type {any} */
  let base = {};
  if (photoPath === source.path && state.recipe) {
    base = { ...state.recipe };
  } else {
    const [sidecar, defaults] = await Promise.all([
      invoke("load_sidecar", { path: source.path }),
      invoke("default_recipe"),
    ]);
    base = { ...defaults, ...(sidecar?.engine_settings ?? {}) };
  }
  // Never copy crop settings across photos
  delete base.crop_aspect;
  delete base.crop_angle;
  delete base.crop_x;
  delete base.crop_y;
  delete base.crop_w;
  delete base.crop_h;
  state.copiedRecipe = base;

  if (notify) notify(`Settings copied from ${source.name}`, 2000);
}

/**
 * Applies a recipe across a set of target frames, preserving each frame's unique crop.
 * @param {{
 *   state: any,
 *   recipeToApply: Recipe | null,
 *   targetFrames: Frame[],
 *   photoPath: string | null,
 *   activity?: any,
 *   startActivity?: (type: string, title: string, total: number) => string,
 *   updateActivity?: (id: string, update: any) => void,
 *   releaseActive?: (id: string) => void,
 *   setProgress?: (p: any) => void,
 *   patchProgress?: (p: any) => void,
 *   advanceProgress?: () => number,
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   freshPreviewVersion?: (path: string) => Promise<number>,
 *   refreshFrames?: () => void,
 *   scheduleRender?: (px: number) => void,
 *   sendDevStateToPanel?: () => void,
 *   notify?: (msg: string, ms?: number) => void,
 *   PREVIEW_PX?: number,
 *   onUpdateLoupeUrl?: (bytes: any) => void,
 * }} params
 */
export async function applyRecipeToFrames({
  state,
  recipeToApply,
  targetFrames,
  photoPath,
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
  scheduleRender: scheduleRenderFn,
  sendDevStateToPanel,
  notify,
  PREVIEW_PX = 2048,
  onUpdateLoupeUrl,
}) {
  if (!recipeToApply || !targetFrames.length || activity?.progress) return;
  const snapshot = { ...recipeToApply };
  if (setProgress) {
    setProgress({ verb: "Applying settings", done: 0, total: targetFrames.length, current: "" });
  }
  const jobId = startActivity
    ? startActivity("develop", `Apply settings to ${targetFrames.length} photo(s)`, targetFrames.length)
    : null;
  try {
    for (const [i, frame] of targetFrames.entries()) {
      if (patchProgress) patchProgress({ current: frame.name });
      if (updateActivity && jobId) updateActivity(jobId, { current: frame.name });

      const existingSidecar = await invoke("load_sidecar", { path: frame.path }).catch(() => null);
      const existingSettings = existingSidecar?.engine_settings ?? {};

      /** @type {Record<string, any>} */
      const frameRecipe = {
        ...snapshot,
        crop_aspect: existingSettings.crop_aspect ?? "original",
        crop_angle: existingSettings.crop_angle ?? 0,
        crop_x: existingSettings.crop_x ?? 0,
        crop_y: existingSettings.crop_y ?? 0,
        crop_w: existingSettings.crop_w ?? 1,
        crop_h: existingSettings.crop_h ?? 1,
      };

      const developing = invoke("develop_preview", {
        path: frame.path,
        recipe: frameRecipe,
        maxPx: PREVIEW_PX,
      });
      const upcoming = targetFrames[i + 1];
      if (upcoming?.path) invoke("prefetch_photo", { path: upcoming.path }).catch(() => {});
      const bytes = await developing;
      await invoke("save_recipe", { path: frame.path, recipe: frameRecipe });
      if (freshPreviewVersion) frame.previewVersion = await freshPreviewVersion(frame.path);
      if (updateActivity && jobId && advanceProgress) {
        updateActivity(jobId, { done: advanceProgress() });
      }

      if (frame.path === photoPath) {
        state.recipe = { ...frameRecipe };
        state.developEngine = frameRecipe.engine === "rapid" ? "rapid" : "spektra";
        if (state.developEngine === "rapid") {
          if (scheduleRenderFn) scheduleRenderFn(PREVIEW_PX);
        } else {
          state.useCanvas = false;
          if (onUpdateLoupeUrl) onUpdateLoupeUrl(bytes);
        }
        if (sendDevStateToPanel) sendDevStateToPanel();
      }
      if (refreshFrames) refreshFrames();
    }
    if (notify) {
      notify(`Settings applied to ${targetFrames.length} photo${targetFrames.length === 1 ? "" : "s"}`, 2500);
    }
    if (updateActivity && jobId) updateActivity(jobId, { phase: "Complete", status: "completed" });
  } catch (e) {
    if (notify) notify(`Could not apply settings: ${e}`);
    if (updateActivity && jobId) updateActivity(jobId, { phase: String(e), status: "failed" });
  } finally {
    if (setProgress) setProgress(null);
    if (releaseActive && jobId) releaseActive(jobId);
  }
}

/**
 * Clears development settings for the photo and restores as-shot preview.
 * @param {{
 *   state: any,
 *   photoPath: string | null,
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   library: { frames: any[] },
 *   freshPreviewVersion?: (path: string) => Promise<number>,
 *   refreshFrames?: () => void,
 *   previewUrl?: (path: string, v: number) => string,
 *   onSetLoupeUrl?: (url: string) => void,
 * }} params
 */
export async function clearDevelopment({
  state,
  photoPath,
  invoke,
  library,
  freshPreviewVersion,
  refreshFrames,
  previewUrl,
  onSetLoupeUrl,
}) {
  if (!photoPath) return;
  const path = photoPath;
  await invoke("clear_recipe", { path });
  if (path !== photoPath) return;
  state.developEngine = null;
  const frame = library.frames.find((item) => item.path === path);
  if (frame && freshPreviewVersion) {
    frame.previewVersion = await freshPreviewVersion(frame.path);
  }
  if (refreshFrames) refreshFrames();

  if (previewUrl && onSetLoupeUrl) {
    const next = previewUrl(path, frame?.previewVersion ?? Date.now());
    const probe = new Image();
    probe.src = next;
    try {
      await probe.decode();
    } catch {
      /* show anyway; onerror handles */
    }
    if (path !== photoPath) return;
    onSetLoupeUrl(next);
  }
  state.useCanvas = false;
  state.status = "";
}

/**
 * Debounced save of photo caption.
 * @param {any} state
 * @param {string | null} photoPath
 * @param {(cmd: string, args?: any) => Promise<any>} invoke
 * @param {(msg: string) => void} [hold]
 * @param {{ current?: ReturnType<typeof setTimeout> }} [timerRef]
 * @param {number} [debounceMs]
 */
export function captionEdited(
  state,
  photoPath,
  invoke,
  hold = undefined,
  timerRef = { current: undefined },
  debounceMs = 400,
) {
  clearTimeout(timerRef.current);
  const path = photoPath;
  const description = state.caption;
  timerRef.current = setTimeout(() => {
    if (path) {
      invoke("save_caption", { path, description }).catch((error) => {
        if (hold) hold(`Could not save caption: ${error}`);
      });
    }
  }, debounceMs);
}

/**
 * Saves tags for the photo.
 * @param {any} state
 * @param {string | null} photoPath
 * @param {(cmd: string, args?: any) => Promise<any>} invoke
 * @param {(msg: string) => void} [hold]
 */
export function tagsEdited(state, photoPath, invoke, hold = undefined) {
  const path = photoPath;
  if (!path) return;
  invoke("save_tags", { path, tags: [...state.tags] }).catch((error) => {
    if (hold) hold(`Could not save tags: ${error}`);
  });
}
