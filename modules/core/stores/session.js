/**
 * Everything Reveal remembers between launches, in one place, read and
 * written by explicit calls.
 *
 * This exists because of a bug that cost an afternoon. The grid always
 * reopened at the top, however far you had scrolled. The offset was saved
 * correctly every single time; an `$effect` that persisted it ran at boot,
 * before anything had been loaded, and wrote out the initial `0` over the
 * real value — which the loader then read back as `0`. Instrumenting every
 * write said it in two lines:
 *
 *     SCROLL save 0
 *     SCROLL restore -> 0
 *
 * Nothing in the code stated an order between saving and loading. It fell
 * out of the reactivity graph, so it was correct by luck and wrong by luck.
 *
 * Hence the shape below. Per-folder memory is reached through
 * {@link openFolderSession}, which hands back the stored values AND the
 * functions that write them. You cannot write a folder's session without
 * having opened it, because until you do, the writer does not exist. The
 * ordering is no longer a rule to remember; it is the only way to type it.
 *
 * Everything here treats storage as hostile: it is arbitrary text a user can
 * edit, it is absent in a plain browser tab, and it throws outright in some
 * private-mode configurations. Every read validates, every read and write is
 * wrapped, and a failure degrades to the default rather than propagating.
 */

/** @returns {Storage | null} */
function store() {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch (_) {
    return null; // private mode can throw on access, not just on use
  }
}

/**
 * @param {string} key
 * @returns {string | null}
 */
function readRaw(key) {
  try {
    return store()?.getItem(key) ?? null;
  } catch (_) {
    return null;
  }
}

/**
 * @param {string} key
 * @param {string} value
 */
function writeRaw(key, value) {
  try {
    store()?.setItem(key, value);
  } catch (_) {
    // A full or blocked quota is not worth failing a photo edit over.
  }
}

/**
 * @template T
 * @param {string} key
 * @param {T} fallback
 * @returns {any}
 */
function readJson(key, fallback) {
  const raw = readRaw(key);
  if (!raw) return fallback;
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : fallback;
  } catch (_) {
    return fallback;
  }
}

/**
 * @param {string} key
 * @param {readonly string[]} allowed
 * @returns {string | null}
 */
function readEnum(key, allowed) {
  const raw = readRaw(key);
  return raw !== null && allowed.includes(raw) ? raw : null;
}

/** Develop's photo size slider — its range, and where it starts. */
export const PHOTO_SIZE_MIN = 40;
export const PHOTO_SIZE_MAX = 200;
/** @type {number} */
export const DEFAULT_PHOTO_SIZE = 75;

/** The two workflow modes, as stored. */
export const MODES = /** @type {const} */ (["cull", "dev"]);

/**
 * Global session values — ones that belong to the app, not to a folder.
 *
 * Each getter validates rather than casts, so a hand-edited or stale entry
 * degrades to the default instead of reaching the UI as a wrong-shaped value.
 */
export const session = {
  /** @returns {string | null} */
  lastMode: () => readEnum("reveal.currentMode", MODES),
  /** @param {string} mode */
  setLastMode: (mode) => writeRaw("reveal.currentMode", mode),

  /** @returns {string | null} */
  lastDirectory: () => readRaw("reveal.lastDirectory"),
  /** @param {string} dir */
  setLastDirectory: (dir) => writeRaw("reveal.lastDirectory", dir),

  /** The photo that was open in Develop when the app last quit.
   * @returns {string | null} */
  lastPhoto: () => readRaw("reveal.lastPhotoPath"),
  /** @param {string} path */
  setLastPhoto: (path) => writeRaw("reveal.lastPhotoPath", path),

  /** Develop's photo size, in percent. Out-of-range or non-numeric → the
   * default, never a size the slider cannot show.
   * @returns {number} */
  photoSize: () => {
    const raw = readRaw("reveal.photoSize");
    const n = raw === null ? NaN : Number(raw);
    return Number.isFinite(n) && n >= PHOTO_SIZE_MIN && n <= PHOTO_SIZE_MAX ? n : DEFAULT_PHOTO_SIZE;
  },
  /** @param {number} percent */
  setPhotoSize: (percent) => writeRaw("reveal.photoSize", String(percent)),

  /** @returns {"uniform" | "masonry" | null} */
  gridLayout: () => /** @type {any} */ (readEnum("reveal.layout", ["uniform", "masonry"])),
  /** @param {string} layout */
  setGridLayout: (layout) => writeRaw("reveal.layout", layout),

  /** Per-mode panel layout. Shape is checked by the caller, which knows it. */
  modeLayouts: () => readJson("reveal.modeLayouts", null),
  /** @param {unknown} layouts */
  setModeLayouts: (layouts) => writeRaw("reveal.modeLayouts", JSON.stringify(layouts)),

  /** Export preferences: edge, border, folder. */
  exportPrefs: () => readJson("reveal.export", null),
  /** @param {unknown} prefs */
  setExportPrefs: (prefs) => writeRaw("reveal.export", JSON.stringify(prefs)),

  /** Grid preferences: cols, margin, aspect, fill, sort. */
  gridPrefs: () => readJson("reveal.grid", null),
  /** @param {unknown} prefs */
  setGridPrefs: (prefs) => writeRaw("reveal.grid", JSON.stringify(prefs)),
};

/**
 * @typedef {Object} FolderSession
 * @property {string} dir The folder this session belongs to.
 * @property {number} scroll Stored viewport offset, 0 when never scrolled.
 * @property {string | null} mode The workflow mode last used here.
 * @property {(offset: number) => void} saveScroll
 * @property {(mode: string) => void} saveMode
 */

/**
 * Read a folder's stored session and, with it, the right to write it.
 *
 * Call this when the folder opens, keep the handle, and persist through it.
 * A folder with no handle is a folder nothing has been read for yet, and
 * nothing may be written for it — which is precisely the invariant that was
 * missing when boot wrote a meaningless `0` over a real scroll offset.
 *
 * @param {string} dir
 * @returns {FolderSession}
 */
export function openFolderSession(dir) {
  const rawScroll = Number(readRaw(`reveal.scroll.${dir}`));
  return {
    dir,
    scroll: Number.isFinite(rawScroll) && rawScroll > 0 ? rawScroll : 0,
    mode: readEnum(`reveal.mode.${dir}`, MODES),
    saveScroll: (offset) => writeRaw(`reveal.scroll.${dir}`, String(offset)),
    saveMode: (mode) => writeRaw(`reveal.mode.${dir}`, mode),
  };
}
