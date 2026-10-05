/**
 * Library state and folder navigation transactions.
 *
 * Encapsulates the open folder, the loaded frames, the catalog roots and tree,
 * and background scanning/indexing progress using Svelte 5 runes.
 */

import { Collection } from "@modules/core";

/** @typedef {{ path: string, name: string, previewVersion?: number, rating?: number, capture_at?: number | string }} Frame */

/**
 * The photos, held RAW on purpose. Deep reactivity on thousands of frames
 * is avoided; we re-publish the array (refreshFrames) when a frame is mutated in place.
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
  /** An open is in flight — suppresses the empty-state splash so switching folders does not flash "REVEAL". */
  loading: false,
  /** Whether a background scan/index is walking the library. */
  scanning: false,
  /** @type {{ dirs: number; frames: number } | null} Progress while a scan walks. */
  indexProgress: null,
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
  get scanning() {
    return state.scanning;
  },
  get indexProgress() {
    return state.indexProgress;
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
 * Direct state manipulation for library module.
 */
export const libraryState = {
  get scanning() {
    return state.scanning;
  },
  set scanning(v) {
    state.scanning = Boolean(v);
  },
  get indexProgress() {
    return state.indexProgress;
  },
  set indexProgress(v) {
    state.indexProgress = v;
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
 */
export function refreshFrames() {
  frames = [...frames];
}

/**
 * Replace the photos of the folder already open.
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
 * Leave the folder without opening another.
 */
export function leaveFolder() {
  state.curDir = null;
  state.folder = null;
  frames = [];
}

/** @param {string | null} dir */
export function setCurDir(dir) {
  state.curDir = dir;
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

/** @param {boolean} value */
export function setScanning(value) {
  state.scanning = Boolean(value);
}

/** @param {{ dirs: number; frames: number } | null} value */
export function setIndexProgress(value) {
  state.indexProgress = value;
}

/**
 * The library root that contains `path` (or is `path`), or null. The longest
 * wins when roots nest.
 * @param {string | null | undefined} path
 * @param {string[]} roots
 * @param {{ strictly?: boolean }} [opts] `strictly`: a root other than `path` itself
 * @returns {string | null}
 */
export function rootCovering(path, roots, { strictly = false } = {}) {
  if (!path) return null;
  let best = null;
  for (const root of roots) {
    if (strictly && root === path) continue;
    if (path === root || path.startsWith(`${root}/`)) {
      if (best === null || root.length > best.length) best = root;
    }
  }
  return best;
}

/**
 * What the "Remove library" confirmation says.
 * @param {string} path
 * @param {string[]} roots every registered library
 * @param {number} [framesCount] how many photos it holds, if known
 * @returns {string}
 */
export function removeLibraryNote(path, roots, framesCount) {
  const outer = rootCovering(path, roots, { strictly: true });
  if (outer) {
    const name = outer.split("/").pop() || outer;
    return `Nothing leaves your library: this folder sits inside “${name}”, which keeps all of its photos. Reveal only stops listing it as a library of its own. No file is touched.`;
  }
  const count = framesCount === undefined ? "photos" : `${framesCount.toLocaleString("en-CA")} photos`;
  return `No file is deleted. Reveal forgets this library and its ${count} from its index. Ratings, captions and tags live in each photo's .xmp file and stories in each folder's note, so adding the folder back and reindexing brings everything back.`;
}

/**
 * Label a directory relative to whichever root owns it.
 * @param {string} dir
 * @returns {string}
 */
export function dirLabel(dir) {
  return Collection.label(dir, state.roots);
}
