/**
 * Which folder is open, and the photos in it.
 *
 * The reason this is a module and not six loose variables: opening a folder
 * is asynchronous, and until now nothing reliably stopped a slow load from
 * landing after you had already moved on.
 *
 * `openDir` guarded its commit with `request !== applePhotosRequest` — a
 * counter belonging to the Apple Photos path, captured on entry and never
 * incremented by `openDir` itself. Between two filesystem folders it
 * therefore compared N against N and always passed: open a large folder,
 * click a small one before the first finished, and the grid could end up
 * showing the first folder's photos under the second folder's name.
 * `openFolder`, the other way in, had no guard at all. The correct check
 * already existed two lines further down (`curDir === dir`) — on the
 * preview-version follow-up, but not on the assignment that mattered.
 *
 * So opening is a transaction here. `beginOpen` moves the app to the new
 * folder and hands back a token; the token is the only way to put photos in,
 * and it refuses once another open has started. A stale load cannot commit
 * because it has nothing to commit through.
 *
 * Same shape as src/lib/session.svelte.js and activity.svelte.js: private
 * state, a read-only view, and writes only through functions that carry the
 * invariant.
 */

/** @typedef {{ path: string, name: string }} Frame */

/**
 * The photos, held RAW on purpose. A folder can hold thousands of frames and
 * deep reactivity would wrap every one of them in a proxy; the app only ever
 * needs to react to the list being replaced. That is also why the code
 * re-publishes the array (see `refreshFrames`) after writing a rating or a
 * preview version onto a frame in place — the mutation itself is invisible.
 * @type {any[]}
 */
let frames = $state.raw([]);

const state = $state({
  /** @type {string[]} Every catalogue root — the library is a SET of them. */
  roots: [],
  /** @type {any[]} The folder tree, as the sidebar shows it. */
  dirs: [],
  /** @type {string | null} An indexed catalogue directory. */
  curDir: null,
  /** @type {string | null} A plain folder, opened outside the catalogue. */
  folder: null,
  /** An open is in flight — suppresses the empty-state splash so switching
   * folders does not flash "REVEAL". */
  loading: false,
});

/**
 * Incremented by every `beginOpen`. A load holding an older number has been
 * overtaken and may not write anything.
 */
let generation = 0;

/** The read-only face. Reactive through the getters. */
export const library = {
  get roots() {
    return state.roots;
  },
  get dirs() {
    return state.dirs;
  },
  get curDir() {
    return state.curDir;
  },
  get folder() {
    return state.folder;
  },
  get frames() {
    return frames;
  },
  get loading() {
    return state.loading;
  },
  /** Whichever of the two is open — what the grid is showing. */
  get dir() {
    return state.curDir ?? state.folder;
  },
  /** The primary root, for the paths that still assume a single one. */
  get root() {
    return state.roots[0] ?? null;
  },
};

/**
 * @typedef {Object} OpenToken
 * @property {boolean} isCurrent Whether this open is still the live one.
 * @property {(rows: any[]) => boolean} commit Put photos in; false if overtaken.
 * @property {(rows: any[]) => boolean} replace Swap the photos for an equal-length, enriched list.
 * @property {() => void} finish Mark the open as settled.
 */

/**
 * Move to a folder and start loading it.
 *
 * Clears the photos immediately, which also unmounts the previous grid cells
 * and cancels their pending thumbnail requests.
 *
 * @param {{ curDir?: string | null, folder?: string | null }} where
 * @returns {OpenToken}
 */
export function beginOpen({ curDir = null, folder = null }) {
  const mine = ++generation;
  state.curDir = curDir;
  state.folder = folder;
  frames = [];
  state.loading = true;

  const current = () => mine === generation;
  return {
    get isCurrent() {
      return current();
    },
    commit(rows) {
      if (!current()) return false;
      frames = rows;
      return true;
    },
    replace(rows) {
      if (!current()) return false;
      frames = [...rows];
      return true;
    },
    finish() {
      if (current()) state.loading = false;
    },
  };
}

/**
 * Add photos to the folder already open — pagination, not a new open.
 * @param {any[]} rows
 */
export function appendFrames(rows) {
  const known = new Set(frames.map((f) => f.path));
  frames = [...frames, ...rows.filter((f) => !known.has(f.path))];
}

/**
 * Re-publish the photo list after mutating a frame in place.
 *
 * A raw `$state` array only reacts to an identity change, so a rating or a
 * preview version written onto an existing frame is invisible until the
 * array is replaced. This is that replacement, named.
 */
export function refreshFrames() {
  frames = [...frames];
}

/**
 * Replace the photos of the folder already open — an index refresh after an
 * import landed in it, not an open of its own. The caller has already checked
 * that this is still the right folder.
 * @param {any[]} rows
 */
export function refreshLoadedFrames(rows) {
  frames = rows;
}

/** Drop the photos without opening anything — leaving a library behind. */
export function clearFrames() {
  frames = [];
}

/**
 * Leave the folder without opening another — after deleting or moving the
 * one that was open. `view` derives from the frames, so clearing both empties
 * the grid on its own.
 */
export function leaveFolder() {
  state.curDir = null;
  state.folder = null;
  frames = [];
}

/** @param {string[]} roots */
export function setRoots(roots) {
  state.roots = roots;
}

/** @param {any[]} dirs */
export function setDirs(dirs) {
  state.dirs = dirs;
}

/** @param {boolean} value */
export function setLoading(value) {
  state.loading = value;
}

/**
 * Label a directory relative to whichever root owns it (the library is
 * multi-root, so the full path is rarely what you want to read).
 * @param {string} dir
 * @returns {string | undefined}
 */
export function dirLabel(dir) {
  const owner = state.roots.find((r) => dir === r || dir.startsWith(r + "/"));
  return owner
    ? dir.slice(owner.length).replace(/^\//, "") || owner.split("/").pop()
    : dir.split("/").pop();
}
