import { invoke } from "@tauri-apps/api/core";
import { activity, startActivity, updateActivity, releaseActive, notify } from "@modules/core";

/**
 * obsidian:// is the reliable way to land on a SPECIFIC note — `open <path>`
 * would just hand the .md file to whatever app owns that extension, which
 * isn't necessarily Obsidian, and wouldn't target the right vault if more than
 * one is registered for it.
 * @param {string} notePath
 */
export function openInObsidian(notePath) {
  invoke("open_path", { path: `obsidian://open?path=${encodeURIComponent(notePath)}` }).catch(() => {});
}

export async function cancelExportQueue() {
  if (!activity.activeId) return;
  updateActivity(activity.activeId, { phase: "Cancelling after current photo…" });
  await invoke("cancel_exports");
}

/**
 * @param {Object} params
 * @param {{ path: string }[]} params.view
 * @param {string} [params.destDir]
 * @param {number} [params.longEdge]
 * @param {number} [params.borderFrac]
 */
export async function exportGrid({ view, destDir = "", longEdge = 2048, borderFrac = 0 }) {
  if (!view.length) return;
  const jobId = startActivity("export", `Export ${view.length} photos`, view.length);
  try {
    const completed = /** @type {number} */ (await invoke("export_photos", {
      paths: view.map((f) => f.path),
      destDir,
      longEdge,
      borderFrac,
    }));
    const job = activity.queue.find((item) => item.id === jobId);
    if (job?.status !== "cancelled") {
      updateActivity(jobId, {
        current: "",
        done: completed,
        phase: completed === view.length ? "Complete" : "Stopped",
        status: completed === view.length ? "completed" : "cancelled",
      });
    }
  } catch (error) {
    updateActivity(jobId, {
      phase: String(error),
      status: "failed",
    });
  } finally {
    releaseActive(jobId);
  }
}

/**
 * The Swift `r` — "Reveal": develop + export the current selection (or the
 * focused frame if nothing is multi-selected) with the active export params.
 * @param {Object} params
 * @param {{ path: string }[]} params.targets
 * @param {string} [params.destDir]
 * @param {number} [params.longEdge]
 * @param {number} [params.borderFrac]
 */
export async function exportSelection({ targets, destDir = "", longEdge = 2048, borderFrac = 0 }) {
  if (!targets.length || activity.anyRunning) return;
  const jobId = startActivity(
    "export",
    `Export ${targets.length} photo${targets.length > 1 ? "s" : ""}`,
    targets.length,
  );
  try {
    const completed = /** @type {number} */ (await invoke("export_photos", {
      paths: targets.map((f) => f.path),
      destDir,
      longEdge,
      borderFrac,
    }));
    updateActivity(jobId, {
      current: "",
      done: completed,
      phase: completed === targets.length ? "Complete" : "Stopped",
      status: completed === targets.length ? "completed" : "cancelled",
    });
  } catch (error) {
    updateActivity(jobId, { phase: String(error), status: "failed" });
  } finally {
    releaseActive(jobId);
  }
}

/**
 * @param {Object} params
 * @param {string | null} params.photoPath
 * @param {any} params.recipe
 * @param {string} [params.picked]
 * @param {string} [params.destDir]
 * @param {number} [params.longEdge]
 * @param {number} [params.borderFrac]
 */
export async function exportCurrent({
  photoPath,
  recipe,
  picked = "",
  destDir = "",
  longEdge = 2048,
  borderFrac = 0,
}) {
  if (!photoPath || !recipe) return;
  const jobId = startActivity("export", `Export ${picked}`, 1);
  try {
    await invoke("export_photo", {
      path: photoPath,
      recipe: { ...recipe },
      destDir,
      longEdge,
      borderFrac,
    });
    updateActivity(jobId, {
      current: picked ?? "",
      done: 1,
      phase: "Complete",
      status: "completed",
    });
    notify("Exported ✓", 2000);
  } catch (e) {
    updateActivity(jobId, { phase: String(e), status: "failed" });
    notify(`Export failed: ${e}`, 5000);
  } finally {
    releaseActive(jobId);
  }
}

/**
 * Develop photo(s) and append to the Obsidian daily note (Logs/yymmdd.md).
 * Filesystem export into the vault attachments folder + daily note append.
 * @param {Object} params
 * @param {string} params.target
 * @param {any} [params.recipe]
 * @param {number} [params.longEdge]
 * @param {number} [params.borderFrac]
 */
export async function exportToDailyNote({ target, recipe = null, longEdge = 2048, borderFrac = 0 }) {
  if (!target) return;
  notify("Sending to the daily note…", 10000);
  try {
    const notePath = /** @type {string} */ (await invoke("export_to_daily_note", {
      path: target,
      recipe: recipe ? { ...recipe } : null,
      longEdge,
      borderFrac,
    }));
    const noteName = notePath.split("/").slice(-2).join("/");
    const filename = target.split("/").pop();
    notify(`Daily note → ${noteName} (${filename}) ✓`, 4000);
    openInObsidian(notePath);
  } catch (e) {
    notify(`Daily note export failed: ${e}`, 5000);
  }
}

/**
 * @param {Object} params
 * @param {string[]} params.targets
 * @param {number} [params.longEdge]
 * @param {number} [params.borderFrac]
 * @param {(target: string) => Promise<void> | void} [params.onSingleExport]
 */
export async function exportSelectionToDailyNote({
  targets,
  longEdge = 2048,
  borderFrac = 0,
  onSingleExport,
}) {
  if (!targets.length) return;

  if (targets.length === 1 && onSingleExport) {
    return onSingleExport(targets[0]);
  }

  const jobId = startActivity("publish", `Daily note · ${targets.length} photos`, targets.length);
  try {
    const notePath = /** @type {string} */ (await invoke("export_batch_to_daily_note", {
      paths: targets,
      longEdge,
      borderFrac,
    }));
    const noteName = notePath.split("/").slice(-2).join("/");
    notify(`Daily note → ${targets.length} photos in ${noteName} ✓`, 4000);
    updateActivity(jobId, { done: targets.length, current: noteName, phase: "Complete", status: "completed" });
    openInObsidian(notePath);
  } catch (e) {
    notify(`Daily note export failed: ${e}`, 5000);
    updateActivity(jobId, { phase: String(e), status: "failed" });
  } finally {
    releaseActive(jobId);
  }
}

/**
 * Unified export dispatcher for any Destination model instance.
 *
 * @param {Object} params
 * @param {import('@modules/core').Destination} params.destination
 * @param {{ path: string }[]} params.targets
 * @param {any} [params.recipe]
 * @param {string} [params.picked]
 * @returns {Promise<any>}
 */
export async function exportToDestination({ destination, targets, recipe, picked }) {
  if (!destination || !targets.length) return;
  const edge = destination.options?.longEdge ?? 2048;
  const borderFrac = destination.options?.border ? (destination.options?.borderFrac ?? 0.04) : 0;

  if (destination.isLocal) {
    if (targets.length === 1 && recipe) {
      return exportCurrent({
        photoPath: targets[0].path,
        recipe,
        picked: picked ?? "",
        destDir: destination.path || "",
        longEdge: edge,
        borderFrac,
      });
    }
    return exportSelection({
      targets,
      destDir: destination.path || "",
      longEdge: edge,
      borderFrac,
    });
  }

  if (destination.isObsidian) {
    if (targets.length === 1) {
      return exportToDailyNote({
        target: targets[0].path,
        recipe,
        longEdge: edge,
        borderFrac,
      });
    }
    return exportSelectionToDailyNote({
      targets: targets.map((t) => t.path),
      longEdge: edge,
      borderFrac,
    });
  }

  if (destination.isGarden) {
    const target = targets[0].path;
    const allowDownload = Boolean(destination.options?.allowDownload);
    const live = await invoke("publish_photo", { path: target, allowDownload });
    return live;
  }

  if (destination.isEditor && destination.path) {
    for (const t of targets) {
      await invoke("open_in_editor", { filePath: t.path, appPath: destination.path });
    }
  }
}
