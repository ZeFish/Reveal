import { invoke } from "@tauri-apps/api/core";
import { notify, hold, setProgress, startActivity, updateActivity, releaseActive, activity } from "@modules/core";
import { storyState } from "./storyState.svelte.js";
import { parseStory, serializeStory, stemOf } from "./storyParser.js";

/**
 * Load the story note for a directory.
 * @param {string | null} dir
 */
export async function loadStory(dir) {
  if (!dir) return;
  storyState.storyContent = /** @type {string} */ (await invoke("load_story_note", { dir }));
}

/**
 * Save story note content to disk.
 * @param {string | null} dir
 * @param {string} content
 * @param {() => Promise<void> | void} [onRefreshed]
 */
export async function saveStoryContent(dir, content, onRefreshed) {
  if (!dir) return;
  try {
    await invoke("save_story_note", { dir, content });
  } catch (e) {
    notify(`Failed to save the story: ${e}`, 8000);
    return;
  }
  const stems = /** @type {string[]} */ (await invoke("story_stems", { dir }));
  storyState.storySet = new Set(stems);
  await onRefreshed?.();
}

/**
 * Refresh story stems and load story content for an open directory.
 * @param {string | null} dir
 */
export async function refreshStory(dir) {
  const stems = dir ? /** @type {string[]} */ (await invoke("story_stems", { dir })) : [];
  storyState.storySet = new Set(stems);
  if (dir) {
    await loadStory(dir);
  }
}

/**
 * Refresh sidebar story indicator dots for all directories.
 * @param {string[] | { dir: string }[]} dirs
 */
export async function refreshStoryDirs(dirs) {
  if (!dirs || !dirs.length) return;
  try {
    const dirList = dirs.map((d) => (typeof d === "string" ? d : d.dir));
    const withStory = /** @type {string[]} */ (await invoke("story_dirs", { dirs: dirList }));
    storyState.storyDirs = new Set(withStory);
  } catch {
    // Quiet fail
  }
}

/**
 * Toggle a photo in/out of the story quick collection.
 * @param {string} path
 * @param {string | null} dir
 * @param {() => Promise<void> | void} [onRefreshed]
 */
export async function toggleStoryWithPath(path, dir, onRefreshed) {
  if (path.startsWith("apple-photos://")) {
    hold("Apple Photos albums are read-only. Use ratings to select photos, then export.");
    return;
  }
  if (!path || !dir) return;
  const updated = /** @type {string[]} */ (await invoke("story_toggle", { dir, path }));
  storyState.storySet = new Set(updated);
  await loadStory(dir);
  await onRefreshed?.();
}

/**
 * Publish the story to Garden.
 * @param {Object} params
 * @param {string | null} params.dir
 * @param {boolean} [params.signedIn]
 */
export async function publishStory({ dir, signedIn = false }) {
  if (!dir || !storyState.storySet.size || !signedIn || activity.anyRunning) return;
  storyState.liveUrl = null;
  const count = storyState.storySet.size;
  setProgress({ verb: "publication", done: 0, total: count, current: "" });
  const publishVerb = storyState.publishVerb;
  const publishTaskId = startActivity("publish", `${publishVerb} story · ${count} photos`, count);
  try {
    const url = /** @type {string} */ (await invoke("publish_story", { dir, dryRun: false }));
    storyState.liveUrl = url;
    notify(storyState.storyPublished ? "updated ✓" : "published ✓", 2000);
    updateActivity(publishTaskId, { done: count, phase: "Complete", status: "completed" });
  } catch (e) {
    notify(`Error: ${e}`, 5000);
    updateActivity(publishTaskId, { phase: String(e), status: "failed" });
  } finally {
    setProgress(null);
    releaseActive(publishTaskId);
  }
}

/**
 * Export story photos to a local folder.
 * @param {Object} params
 * @param {string | null} params.dir
 * @param {number} [params.longEdge]
 * @param {number} [params.borderFrac]
 */
export async function exportLocalStory({ dir, longEdge = 2048, borderFrac = 0 }) {
  if (!dir || !storyState.storySet.size || activity.anyRunning) return;
  const dest = /** @type {string | null} */ (await invoke("pick_folder"));
  if (!dest) return;
  storyState.liveUrl = null;
  const count = storyState.storySet.size;
  setProgress({ verb: "export", done: 0, total: count, current: "" });
  const jobId = startActivity("export", `Export story · ${count} photos`, count);
  try {
    await invoke("export_local_story", {
      dir,
      dest,
      longEdge,
      borderFrac,
    });
    notify("exported ✓", 2000);
    updateActivity(jobId, { done: count, current: dest, phase: "Complete", status: "completed" });
  } catch (e) {
    notify(`Error: ${e}`, 5000);
    updateActivity(jobId, { phase: String(e), status: "failed" });
  } finally {
    setProgress(null);
    releaseActive(jobId);
  }
}

/**
 * Check if the current folder story is published on Garden.
 * @param {string | null} dir
 * @param {boolean} [signedIn]
 */
export async function checkPublishStatus(dir, signedIn = false) {
  if (!dir || storyState.storySet.size === 0 || !signedIn) {
    storyState.storyRemote = null;
    return;
  }
  try {
    const status = /** @type {any} */ (await invoke("story_publish_status", { dir }));
    storyState.storyRemote = status;
  } catch {
    storyState.storyRemote = null;
  }
}

/**
 * Build prose map keyed by grid row.
 * @param {any[]} storyBlocks
 * @param {{ name?: string, path?: string }[]} view
 * @param {number} cols
 * @returns {Map<number, {id: string, text: string}[]>}
 */
export function buildGridProse(storyBlocks, view, cols) {
  /** @type {Map<number, {id: string, text: string}[]>} */
  const map = new Map();
  const rowOf = new Map(view.map((f, i) => [stemOf(f.name || f.path?.split("/").pop() || ""), Math.floor(i / cols)]));
  let anchorRow = -1;
  for (const b of storyBlocks) {
    if (b.isPhoto) {
      const row = rowOf.get(b.stem);
      if (row !== undefined) anchorRow = row;
    } else if (b.text && b.text.trim()) {
      const explicitRow = b.stem ? rowOf.get(b.stem) : undefined;
      const row = explicitRow !== undefined ? explicitRow : anchorRow;
      const list = map.get(row) ?? [];
      list.push({ id: b.id, text: b.text });
      map.set(row, list);
    }
  }
  return map;
}

/**
 * Save an inline prose block inserted from the Grid view.
 * @param {Object} params
 * @param {string | null} params.dir
 * @param {number} params.row
 * @param {string} params.text
 * @param {string} [params.blockId]
 * @param {{ name: string }[]} params.view
 * @param {number} params.cols
 * @param {() => Promise<void> | void} [params.onRefreshed]
 */
export async function saveGridProse({ dir, row, text, blockId, view, cols, onRefreshed }) {
  if (!dir) return;
  const trimmed = (text ?? "").trim();
  const { frontmatter, blocks } = parseStory(storyState.storyContent);

  if (blockId) {
    const i = blocks.findIndex((b) => b.id === blockId);
    if (i === -1) return;
    if (!trimmed) blocks.splice(i, 1);
    else blocks[i] = { ...blocks[i], text: trimmed };
  } else {
    if (!trimmed) return;
    let displayStem = null;
    let fileAnchorStem = null;
    for (let i = 0; i < view.length; i++) {
      if (Math.floor(i / cols) > row) break;
      const s = stemOf(view[i].name);
      displayStem = s;
      if (storyState.storySet.has(s)) fileAnchorStem = s;
    }
    const anchorIdx = fileAnchorStem ? blocks.findIndex((b) => b.isPhoto && b.stem === fileAnchorStem) : -1;
    blocks.splice(anchorIdx + 1, 0, {
      id: `blk-grid-${Date.now()}`,
      isPhoto: false,
      stem: displayStem ?? "",
      text: trimmed,
      rowBreak: true,
    });
  }

  const next = serializeStory(frontmatter, blocks);
  await saveStoryContent(dir, next, onRefreshed);
  storyState.storyContent = next;
}

/**
 * Watches and schedules debounced publication status checks.
 * @param {string | null | undefined} dir
 * @param {{
 *   isTauri: boolean,
 *   count: number,
 *   signedIn: boolean,
 *   checkStatus?: (dir: string, signedIn: boolean) => Promise<any>,
 *   delayMs?: number,
 * }} opts
 * @returns {() => void}
 */
export function watchPublishStatus(dir, {
  isTauri,
  count,
  signedIn,
  checkStatus = checkPublishStatus,
  delayMs = 400,
}) {
  if (!isTauri || !dir || count === 0 || !signedIn) {
    storyState.storyRemote = null;
    return () => {};
  }
  let stale = false;
  const timer = setTimeout(async () => {
    if (!stale) await checkStatus(dir, signedIn);
  }, delayMs);
  return () => {
    stale = true;
    clearTimeout(timer);
  };
}

