/**
 * Library operations and filesystem transactions.
 *
 * Implements folder tree scanning, photo relocation, library adding/removal,
 * folder creation/renaming, and preview version fetching.
 */

import {
  setRoots,
  setDirs,
  leaveFolder,
} from "./libraryState.svelte.js";

/**
 * Prompt the user to pick a folder and index it as a new catalogue root.
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void,
 *   refreshDirs: () => Promise<boolean>,
 *   openDir?: (dir: string) => void,
 *   state?: { scanning: boolean },
 *   library?: { dirs: any[] },
 * }} options
 */
export async function indexRoot({ invoke, notify, refreshDirs, openDir, state, library }) {
  const path = await invoke("pick_folder");
  if (!path) return;
  if (state) state.scanning = true;
  try {
    await invoke("scan_root", { path });
    await refreshDirs();
    if (library?.dirs?.length && openDir) {
      openDir(library.dirs[library.dirs.length - 1].dir);
    }
  } catch (error) {
    if (notify) notify(String(error), 9000);
  } finally {
    if (state) state.scanning = false;
  }
}

/**
 * List every registered library with frame counts and mounted state.
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   log?: (msg: string) => void,
 * }} options
 */
export async function listLibraries({ invoke, log }) {
  try {
    return await invoke("catalog_roots");
  } catch (error) {
    if (log) log(`libraries unavailable: ${error}`);
    return [];
  }
}

/**
 * Forget a library from the catalogue index without touching files on disk.
 * @param {string} path
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void,
 *   refreshDirs: () => Promise<boolean>,
 *   curDir?: string | null,
 * }} options
 */
export async function removeLibrary(path, { invoke, notify, refreshDirs, curDir }) {
  const pruned = await invoke("remove_catalog_root", { path });
  if (curDir === path || curDir?.startsWith(`${path}/`)) {
    leaveFolder();
  }
  await refreshDirs();
  if (notify) {
    notify(`Library removed · ${Number(pruned).toLocaleString("en-CA")} photos forgotten`, 4000);
  }
  return pruned;
}

/**
 * Rescans a specific catalog root.
 * @param {string} path
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   refreshDirs: () => Promise<boolean>,
 *   state?: { scanning: boolean },
 * }} options
 */
export async function rescanLibrary(path, { invoke, refreshDirs, state }) {
  if (state) state.scanning = true;
  try {
    await invoke("scan_root", { path });
    await refreshDirs();
  } finally {
    if (state) state.scanning = false;
  }
}

/**
 * Rescans all library roots or delegates to Apple Photos if active.
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   refreshDirs: () => Promise<boolean>,
 *   openDir?: (dir: string, restoreMode?: boolean, restoreSession?: boolean, keepFilters?: boolean) => Promise<void> | void,
 *   state?: { scanning: boolean },
 *   library?: { roots: string[], curDir: string | null, dirs: any[] },
 *   isApplePhotosActive?: boolean,
 *   refreshApplePhotos?: (cb?: any) => Promise<void>,
 *   handleRefreshActive?: any,
 * }} options
 */
export async function rescan({
  invoke,
  refreshDirs,
  openDir,
  state,
  library,
  isApplePhotosActive = false,
  refreshApplePhotos,
  handleRefreshActive,
}) {
  if (isApplePhotosActive && refreshApplePhotos) {
    await refreshApplePhotos(handleRefreshActive);
    return;
  }
  if (!library?.roots?.length) return;
  if (state) state.scanning = true;
  try {
    for (const r of library.roots) {
      await invoke("scan_root", { path: r });
    }
    await refreshDirs();
    if (openDir) {
      if (library.curDir) {
        await openDir(library.curDir, true, false, true);
      } else if (library.dirs.length) {
        await openDir(library.dirs[library.dirs.length - 1].dir);
      }
    }
  } finally {
    if (state) state.scanning = false;
  }
}

/**
 * Reindexes a single directory.
 * @param {string} path
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   refreshDirs: () => Promise<boolean>,
 *   openDir?: (dir: string, restoreMode?: boolean, restoreSession?: boolean, keepFilters?: boolean) => Promise<void> | void,
 *   hold?: (msg: string) => void,
 *   dismiss?: () => void,
 *   activityMessage?: string,
 *   state?: { scanning: boolean },
 *   roots?: string[],
 *   curDir?: string | null,
 * }} options
 */
export async function rescanDir(path, {
  invoke,
  refreshDirs,
  openDir,
  hold,
  dismiss,
  activityMessage,
  state,
  roots = [],
  curDir = null,
}) {
  if (!path || state?.scanning) return;
  if (state) state.scanning = true;
  const name = path.split("/").pop();
  if (hold) hold(`Reindexing ${name}…`);
  try {
    await invoke(roots.includes(path) ? "scan_root" : "scan_folder", { path });
    await refreshDirs();
    if (curDir?.startsWith(path) && openDir) {
      await openDir(curDir, true, false, true);
    }
    if (hold) hold(`Reindexed ${name}`);
  } catch (error) {
    if (hold) hold(`Could not reindex folder: ${error}`);
  } finally {
    if (state) state.scanning = false;
    if (dismiss) {
      setTimeout(() => {
        if (activityMessage?.startsWith("Reindexed ")) dismiss();
      }, 2200);
    }
  }
}

/**
 * Reveals a directory in the OS file manager.
 * @param {string} path
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   hold?: (msg: string) => void,
 * }} options
 */
export async function revealDir(path, { invoke, hold }) {
  try {
    await invoke("open_path", { path });
  } catch (error) {
    if (hold) hold(`Could not open folder: ${error}`);
  }
}

/**
 * Prepares DragEvent payload when dragging photos to a folder.
 * @param {string} path
 * @param {DragEvent} event
 * @param {{
 *   selectedPaths?: Set<string>,
 *   hold?: (msg: string) => void,
 * }} options
 */
export function onPhotoDragStart(path, event, { selectedPaths = new Set(), hold } = {}) {
  if (path.startsWith("apple-photos://")) {
    event.preventDefault();
    if (hold) hold("Export Apple Photos before moving them to a folder.");
    return;
  }
  if (!event.dataTransfer) return;
  const paths = selectedPaths.has(path) && selectedPaths.size > 1 ? [...selectedPaths] : [path];
  const payload = JSON.stringify(paths);
  event.dataTransfer.setData("application/x-reveal-photos", payload);
  event.dataTransfer.setData("text/plain", payload);
  event.dataTransfer.effectAllowed = "move";

  const target = /** @type {any} */ (event.currentTarget || event.target);
  const isTargetImg = typeof HTMLImageElement !== "undefined" && event.target instanceof HTMLImageElement;
  const img = target?.querySelector?.("img") || (isTargetImg ? event.target : null);
  if (img && typeof HTMLImageElement !== "undefined" && img instanceof HTMLImageElement && event.dataTransfer.setDragImage) {
    event.dataTransfer.setDragImage(img, Math.round(img.offsetWidth / 2), Math.round(img.offsetHeight / 2));
  }
}

/**
 * Moves photos to another folder and reconciles the catalogue index.
 * @param {string[]} paths
 * @param {string} destDir
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void,
 *   hold?: (msg: string) => void,
 *   setProgress?: (p: any) => void,
 *   startActivity?: (action: string, label: string, total: number) => string,
 *   updateActivity?: (id: string, update: any) => void,
 *   releaseActive?: (id: string) => void,
 *   refreshDirs: () => Promise<boolean>,
 *   curDir?: string | null,
 *   openDir?: (dir: string, restoreMode?: boolean, restoreSession?: boolean, keepFilters?: boolean) => Promise<void> | void,
 *   clearSelection?: () => void,
 * }} options
 */
export async function movePhotos(paths, destDir, {
  invoke,
  notify,
  hold,
  setProgress,
  startActivity,
  updateActivity,
  releaseActive,
  refreshDirs,
  curDir,
  openDir,
  clearSelection,
}) {
  if (!destDir || !paths?.length) return;
  /** @param {string} p */
  const parentOf = (p) => p.slice(0, p.lastIndexOf("/"));
  const toMove = paths.filter((p) => parentOf(p) !== destDir);
  if (!toMove.length) {
    if (notify) notify("already in this folder", 2500);
    return;
  }
  const srcDirs = new Set(toMove.map(parentOf));
  if (setProgress) setProgress({ verb: "move", done: 0, total: toMove.length, current: "" });
  const jobId = startActivity ? startActivity("move", `Moving ${toMove.length} photo(s)`, toMove.length) : null;
  let moved = 0;
  const errors = [];
  for (const p of toMove) {
    try {
      await invoke("move_photo", { path: p, destDir });
      moved += 1;
      if (setProgress) setProgress({ verb: "move", done: moved, total: toMove.length, current: p.split("/").pop() });
      if (updateActivity && jobId) updateActivity(jobId, { done: moved, current: p.split("/").pop() });
    } catch (e) {
      errors.push(`${p.split("/").pop()} : ${e}`);
    }
  }

  const destName = destDir.split("/").pop();
  try {
    try {
      await invoke("scan_folder", { path: destDir });
      for (const d of srcDirs) await invoke("scan_folder", { path: d });
    } catch (_) {}
    await refreshDirs();
    if (curDir && openDir) await openDir(curDir, true, false, true);
    if (clearSelection) clearSelection();
  } finally {
    if (setProgress) setProgress(null);
    if (hold) {
      hold(
        errors.length
          ? `${moved} moved · ${errors.length} failed`
          : `${moved} photo${moved > 1 ? "s" : ""} moved → ${destName}`,
      );
    }
    if (updateActivity && jobId) {
      updateActivity(jobId, {
        current: destName,
        phase: errors.length ? `${errors.length} failed` : "Complete",
        status: errors.length ? "failed" : "completed",
      });
    }
    if (releaseActive && jobId) releaseActive(jobId);
  }
  if (errors.length) console.warn("move errors:", errors);
}

/**
 * Reconciles the index after a folder creation, rename, or relocation.
 * @param {string} newPath
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   refreshDirs: () => Promise<boolean>,
 * }} options
 */
export async function reconcileAfterFileOp(newPath, { invoke, refreshDirs }) {
  try {
    await invoke("scan_folder", { path: newPath });
  } catch (_) {}
  await refreshDirs();
}

/**
 * Renames a folder on disk and adjusts the catalogue index.
 * @param {string} path
 * @param {string} newName
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void,
 *   refreshDirs: () => Promise<boolean>,
 *   curDir?: string | null,
 *   openDir?: (dir: string) => Promise<void> | void,
 * }} options
 */
export async function renameDir(path, newName, { invoke, notify, refreshDirs, curDir, openDir }) {
  try {
    const newPath = await invoke("rename_dir", { path, newName });
    await reconcileAfterFileOp(newPath, { invoke, refreshDirs });
    if (openDir) {
      if (curDir === path) {
        await openDir(newPath);
      } else if (curDir && curDir.startsWith(path + "/")) {
        await openDir(newPath + curDir.slice(path.length));
      }
    }
    if (notify) notify(`Renamed → ${newName}`, 3000);
    return newPath;
  } catch (e) {
    if (notify) notify(`Rename failed: ${e}`, 3000);
    return null;
  }
}

/**
 * Creates a new directory on disk.
 * @param {string} parentDir
 * @param {string} name
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void,
 * }} options
 */
export async function createFolder(parentDir, name, { invoke, notify }) {
  try {
    const abs = await invoke("create_dir", { parentDir, name });
    if (notify) notify(`Folder created: ${name}`, 2500);
    return abs;
  } catch (e) {
    if (notify) notify(`Creation failed: ${e}`, 4000);
    return null;
  }
}

/**
 * Moves a folder directory on disk and adjusts index.
 * @param {string} path
 * @param {string} destParentDir
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void,
 *   refreshDirs: () => Promise<boolean>,
 *   curDir?: string | null,
 *   openDir?: (dir: string) => Promise<void> | void,
 * }} options
 */
export async function moveDir(path, destParentDir, { invoke, notify, refreshDirs, curDir, openDir }) {
  const name = path.split("/").pop();
  try {
    const newPath = await invoke("move_dir", { path, destParentDir });
    await reconcileAfterFileOp(newPath, { invoke, refreshDirs });
    if (openDir) {
      if (curDir === path) {
        await openDir(newPath);
      } else if (curDir && curDir.startsWith(path + "/")) {
        await openDir(newPath + curDir.slice(path.length));
      }
    }
    if (notify) notify(`${name} moved`, 3000);
    return newPath;
  } catch (e) {
    if (notify) notify(`Move failed: ${e}`, 3000);
    return null;
  }
}

/**
 * Fetches preview version numbers for thumbnail cache busting.
 * @param {any[]} rows
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   isTauri?: boolean,
 * }} options
 */
export async function withPreviewVersions(rows, { invoke, isTauri = true }) {
  if (!isTauri || !Array.isArray(rows) || !rows.length) return rows;
  const batchSize = Math.min(rows.length, 500);
  try {
    const versions = await invoke("preview_versions", {
      paths: rows.slice(0, batchSize).map((r) => r.path),
    });
    for (let i = 0; i < versions.length; i++) rows[i].previewVersion = versions[i] ?? 0;
  } catch (_) {
    // Non-fatal — thumbs just fall back to version 0.
  }
  return rows;
}

/**
 * Returns latest preview version timestamp for a single photo.
 * @param {string} path
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function freshPreviewVersion(path, { invoke }) {
  try {
    const [v] = /** @type {number[]} */ (await invoke("preview_versions", { paths: [path] }));
    return v || Date.now();
  } catch {
    return Date.now();
  }
}

/**
 * Refreshes directory list and catalog roots from backend index.
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   refreshStoryDirs?: () => Promise<void> | void,
 *   log?: (msg: string) => void,
 *   light?: boolean,
 * }} options
 */
export async function refreshDirs({ invoke, refreshStoryDirs, log, light = false }) {
  try {
    const [r, d] = await invoke("index_dirs");
    setRoots(r);
    setDirs(d);
    if (!light && refreshStoryDirs) {
      await refreshStoryDirs();
    }
    return true;
  } catch (error) {
    if (log) log(`folder tree unavailable: ${error}`);
    return false;
  }
}

/**
 * Opens a folder picker dialog.
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   openFolder?: (path: string) => void,
 * }} options
 */
export async function pickFolder({ invoke, openFolder }) {
  const path = await invoke("pick_folder");
  if (path && openFolder) openFolder(path);
  return path;
}

/**
 * Loads markdown catalog note.
 * @param {string} root
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function loadCatalogNote(root, { invoke }) {
  if (!root) return "";
  return await invoke("load_catalog_note", { root });
}

/**
 * Saves markdown catalog note.
 * @param {string} root
 * @param {string} content
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function saveCatalogNote(root, content, { invoke }) {
  if (!root) return;
  await invoke("save_catalog_note", { root, content });
}
