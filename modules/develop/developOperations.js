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
 * Hand a recipe to the backend to be written to the photo's sidecar.
 *
 * Not written here: the backend (photo_writes.rs) keeps the LATEST recipe per photo, writes it
 * once the photo has been left alone, retries what a NAS recovers from, flushes it before the
 * photo's sidecar is read and when the app closes, and says so if it finally fails. All that is
 * left to do here is the hand-over — which only fails if the backend cannot be reached.
 *
 * @param {string} path
 * @param {any} snapshot
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any>, hold?: (msg: string) => void }} context
 */
export function queueRecipeSave(path, snapshot, { invoke, hold }) {
  invoke("queue_save_recipe", { path, recipe: snapshot }).catch((/** @type {any} */ error) => {
    if (hold) hold(`Could not save development settings: ${error}`);
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
      queueRecipeSave(path, snapshot, { invoke, hold });
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
  const CROP_KEYS = ["crop_aspect", "crop_angle", "crop_x", "crop_y", "crop_w", "crop_h"];

  // The photo on screen answers first: it takes the new settings (keeping its own crop) and
  // renders now, before anyone has talked to the NAS about the other photos.
  const onScreen = photoPath ? targetFrames.find((f) => f.path === photoPath) : undefined;
  if (onScreen && state.recipe) {
    const own = Object.fromEntries(CROP_KEYS.filter((k) => k in state.recipe).map((k) => [k, state.recipe[k]]));
    state.recipe = { ...snapshot, ...own };
    state.developEngine = snapshot.engine === "rapid" ? "rapid" : "spektra";
    if (state.developEngine !== "rapid") state.useCanvas = false;
    if (scheduleRenderFn) scheduleRenderFn(PREVIEW_PX);
    if (sendDevStateToPanel) sendDevStateToPanel();
  }

  if (setProgress) {
    setProgress({ verb: "Applying settings", done: 0, total: targetFrames.length, current: "" });
  }
  const jobId = startActivity
    ? startActivity("develop", `Apply settings to ${targetFrames.length} photo(s)`, targetFrames.length)
    : null;
  let failed = 0;
  /** @type {unknown} */
  let firstError = null;

  /** @param {any} frame */
  async function applyOne(frame) {
    if (patchProgress) patchProgress({ current: frame.name });
    if (updateActivity && jobId) updateActivity(jobId, { current: frame.name });
    try {
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
      if (frame !== onScreen) {
        // Rendering publishes the grid preview (in the background — the answer does not wait
        // for the NAS). The photo on screen is rendered by the pump above instead.
        await invoke("develop_preview", { path: frame.path, recipe: frameRecipe, maxPx: PREVIEW_PX });
      }
      await invoke("save_recipe", { path: frame.path, recipe: frameRecipe });
    } catch (e) {
      failed += 1;
      firstError ??= e;
    } finally {
      if (updateActivity && jobId && advanceProgress) updateActivity(jobId, { done: advanceProgress() });
    }
  }

  try {
    // Two at a time: each photo is partly waiting on the NAS (read the sidecar, write the
    // recipe), so a second one fills that wait; more would only hold several decoded frames
    // in memory at once (a 100-megapixel RAW is over a gigabyte once decoded).
    const WORKERS = 2;
    let next = 0;
    const next_frame = () => (next < targetFrames.length ? targetFrames[next++] : null);
    const prefetched = new Set();
    const worker = async () => {
      for (let frame = next_frame(); frame; frame = next_frame()) {
        const upcoming = targetFrames[next];
        if (upcoming?.path && !prefetched.has(upcoming.path)) {
          prefetched.add(upcoming.path);
          invoke("prefetch_photo", { path: upcoming.path }).catch(() => {});
        }
        await applyOne(frame);
      }
    };
    await Promise.all(Array.from({ length: Math.min(WORKERS, targetFrames.length) }, worker));
    if (refreshFrames) refreshFrames();
    if (failed && notify) {
      notify(`Could not apply settings to ${failed} photo${failed === 1 ? "" : "s"}: ${firstError}`);
    } else if (notify) {
      notify(`Settings applied to ${targetFrames.length} photo${targetFrames.length === 1 ? "" : "s"}`, 2500);
    }
    if (updateActivity && jobId) {
      updateActivity(jobId, failed ? { phase: String(firstError), status: "failed" } : { phase: "Complete", status: "completed" });
    }
  } finally {
    if (setProgress) setProgress(null);
    if (releaseActive && jobId) releaseActive(jobId);
  }
}

/**
 * Show the photo as the camera shot it — NOW. Nothing is read from or written to the photo's
 * folder: the camera's own picture is asked for directly (`asShot`), so the switch to "None"
 * never waits on clearing the develop settings off the NAS (the backend clears them later, and
 * only if the person stays on None — see photo_writes.rs).
 *
 * @param {{
 *   state: any,
 *   photoPath: string | null,
 *   previewUrl?: (path: string, version?: number, asShot?: boolean) => string,
 *   onSetLoupeUrl?: (url: string) => void,
 * }} o
 */
export async function showAsShot({ state, photoPath, previewUrl, onSetLoupeUrl }) {
  if (!photoPath) return;
  const path = photoPath;
  state.developEngine = null;
  state.useCanvas = false;
  state.status = "";
  if (!previewUrl || !onSetLoupeUrl) return;
  const next = previewUrl(path, Date.now(), true);
  const probe = new Image();
  probe.src = next;
  try {
    await probe.decode();
  } catch {
    /* show anyway; onerror handles */
  }
  // Only if the person has not chosen an engine again while the picture was decoding.
  if (state.developEngine === null) onSetLoupeUrl(next);
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
