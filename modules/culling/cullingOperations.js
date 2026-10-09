import { invoke } from "@tauri-apps/api/core";
import { notify, hold, setProgress, startActivity, updateActivity, releaseActive, activity, selectedFrames } from "@modules/core";
import { refreshFrames } from "@modules/library";
import { storyState, stemOf } from "@modules/story";
import { cullingState, MASONRY_LIMIT } from "./cullingState.svelte.js";

/**
 * Persist the current grid geometry and sorting preferences to session.
 * @param {any} session
 */
export function saveGridPrefs(session) {
  session?.setGridPrefs({
    cols: cullingState.cols,
    marginScale: cullingState.marginScale,
    cellAspect: cullingState.cellAspect,
    fillCells: cullingState.fillCells,
    sortDesc: cullingState.sortDesc,
  });
}

/**
 * Toggle layout between uniform and masonry, with ceiling limit check.
 * @param {Object} params
 * @param {number} [params.totalFrames]
 * @param {any} [params.session]
 */
export function toggleLayout({ totalFrames = 0, session } = {}) {
  if (cullingState.layout === "uniform" && totalFrames > MASONRY_LIMIT) {
    notify(`Masonry draws every photo at once — ${totalFrames} is past the limit of ${MASONRY_LIMIT}`, 4000);
    return;
  }
  cullingState.layout = cullingState.layout === "uniform" ? "masonry" : "uniform";
  session?.setGridLayout(cullingState.layout);
}

/**
 * Set star rating on all currently selected frames.
 * @param {any[]} view
 * @param {number} n
 */
export async function rate(view, n) {
  const targets = selectedFrames(view);
  if (!targets.length) return;
  // Optimistic: the stars change on screen now; the sidecar write (a NAS round trip per
  // photo) follows. A failure puts the old rating back and says so.
  const before = targets.map((frame) => frame.rating);
  const targetPaths = new Set(targets.map((frame) => frame.path));
  for (const frame of targets) frame.rating = n;
  refreshFrames(targetPaths);
  /** @type {any[]} */
  const unsaved = [];
  for (const [i, frame] of targets.entries()) {
    try {
      await invoke("set_rating", { path: frame.path, rating: n });
    } catch (error) {
      frame.rating = before[i];
      unsaved.push(error);
    }
  }
  if (unsaved.length) {
    hold(`Could not save photo rating: ${unsaved[0]}`);
    refreshFrames(targetPaths);
  }
}

/**
 * Cancel an ongoing AI culling operation.
 */
export function stopCull() {
  invoke("cancel_cull").catch(() => {});
}

/**
 * Cull one imported folder down to target frames via vision ranking.
 * @param {string} dir
 * @param {string[]} paths
 * @param {() => Promise<void> | void} [onStale]
 */
export async function triggerAiCull(dir, paths, onStale) {
  try {
    await invoke("ai_cull", { dir, paths });
    await onStale?.();
  } catch (error) {
    notify(`AI Culling (${dir.split("/").pop()}) : ${error}`, 6000);
  }
}

/**
 * Run AI culling on the current folder's view and add picks to quick collection.
 * @param {Object} params
 * @param {string | null} params.dir
 * @param {{ path: string, name: string }[]} params.view
 * @param {boolean} [params.isApplePhotos]
 * @param {() => Promise<void> | void} [params.onStoryRefreshed]
 */
export async function cullCurrentFolder({ dir, view, isApplePhotos = false, onStoryRefreshed }) {
  if (isApplePhotos) {
    hold("AI culling is available for filesystem folders, not Apple Photos.");
    return;
  }
  if (!dir || !view.length || activity.progress) return;
  setProgress({ verb: "cull", done: 0, total: view.length, current: "" });
  hold(`AI Culling · ${view.length} photos…`);
  const taskId = startActivity("cull", `AI Culling · ${view.length} photos`, view.length);
  cullingState.cullTaskId = taskId;
  try {
    const result = /** @type {{ picked: string[], considered: number }} */ (
      await invoke("ai_cull_selection", { dir, paths: view.map((f) => f.path) })
    );
    const currentStems = new Set(/** @type {string[]} */ (await invoke("story_stems", { dir })));
    let added = 0;
    for (const path of result.picked) {
      const s = stemOf(path.split("/").pop() || "");
      if (currentStems.has(s)) continue;
      const updated = /** @type {string[]} */ (await invoke("story_toggle", { dir, path }));
      storyState.storySet = new Set(updated);
      currentStems.add(s);
      added++;
    }
    await onStoryRefreshed?.();
    notify(`AI Culling ✓ ${added} added to the quick collection (${result.picked.length}/${result.considered} kept)`, 6000);
    updateActivity(taskId, {
      done: result.picked.length,
      total: result.considered,
      current: `${added} added`,
      phase: "Complete",
      status: "completed",
    });
  } catch (error) {
    const stopped = String(error) === "cancelled";
    notify(stopped ? "AI Culling stopped" : `AI Culling : ${error}`, 6000);
    updateActivity(
      taskId,
      stopped ? { phase: "Stopped", status: "cancelled" } : { phase: String(error), status: "failed" },
    );
  } finally {
    setProgress(null);
    releaseActive(taskId);
    if (cullingState.cullTaskId === taskId) cullingState.cullTaskId = null;
  }
}

/**
 * Handler for backend cull-started event.
 * @param {any} payload
 */
export function handleCullStarted(payload) {
  const total = payload?.total ?? 1;
  cullingState.cullTaskId = startActivity("cull", `AI Culling · ${payload?.total ?? "?"} photos`, total);
}

/**
 * Handler for backend cull-progress event.
 * @param {any} payload
 */
export function handleCullProgress(payload) {
  const phaseLabel = payload?.phase === "cloud" ? "visual analysis" : "local sort";
  setProgress({ verb: "cull", done: payload?.done, total: payload?.total, current: phaseLabel });
  if (cullingState.cullTaskId) {
    updateActivity(cullingState.cullTaskId, { current: phaseLabel, done: payload?.done, total: payload?.total });
  }
}

/**
 * Handler for backend cull-finished event.
 * @param {any} payload
 */
export function handleCullFinished(payload) {
  const stats = payload || {};
  const outcome = stats.exported_to
    ? (stats.marked > 0 ? `added to story, exported → ${stats.exported_to}` : `exported → ${stats.exported_to}`)
    : (stats.marked > 0 ? "added to story" : "kept");
  notify(`AI Culling ✓ ${stats.picked}/${stats.considered} kept · ${outcome}`, 6000);
  if (activity.progress?.verb === "cull") setProgress(null);
  if (cullingState.cullTaskId) {
    updateActivity(cullingState.cullTaskId, {
      done: stats.picked,
      total: stats.considered,
      current: outcome,
      phase: "Complete",
      status: "completed",
    });
    releaseActive(cullingState.cullTaskId);
    cullingState.cullTaskId = null;
  }
}

/**
 * Handler for backend cull-failed event.
 * @param {any} payload
 */
export function handleCullFailed(payload) {
  notify(payload?.message === "cancelled" ? "AI Culling stopped" : `AI Culling : ${payload?.message}`, 6000);
  if (activity.progress?.verb === "cull") setProgress(null);
  if (cullingState.cullTaskId) {
    updateActivity(cullingState.cullTaskId, { phase: String(payload?.message), status: "failed" });
    releaseActive(cullingState.cullTaskId);
    cullingState.cullTaskId = null;
  }
}
