/**
 * Which photos are chosen, and which one has the focus.
 *
 * Held as PATHS, never as positions. They used to be indexes into `view`,
 * and `view` is not one list — it filters by rating and by story, and
 * reverses under `sortDesc`. An index only means something beside the list
 * it was computed in, and two bugs came straight out of that: restoring a
 * session looked the saved photo up in `frames` and assigned the result to
 * `sel`, which on a 61-photo folder reopened the mirror photo (positions 16
 * and 46) every other launch; and flipping the sort left the index alone,
 * quietly moving the selection to the opposite end of the folder.
 *
 * Every function here therefore takes `view` as an explicit argument. That
 * is the whole design: you cannot ask where the focus is without saying
 * which list you mean, so an index from the wrong list stops being a thing
 * you can write. The page keeps `const sel = $derived(positionIn(view))`.
 *
 * The state is module-private and exported read-only, for the same reason
 * `activity.svelte.js` is: shared state with uncoordinated writers is the
 * shape all of today's bugs had.
 */

const state = $state({
  /** @type {string | null} */ focusPath: null,
  /** @type {string | null} */ anchorPath: null,
  /**
   * Where to land when the focused photo is not in `view` at all — filtered
   * out by a rating, hidden by Editorial, moved or deleted. Keeping the last
   * position means the grid stays where you were working rather than
   * jumping to the top.
   */
  fallback: 0,
  /** @type {Set<string>} */ paths: new Set(),
});

/**
 * Anything with a `path`. Generic on purpose: the page's own frame type
 * carries a rating, a preview version and more, and `selectedFrames` must
 * hand those back unchanged rather than flattening them to this shape.
 * @typedef {{ path: string }} HasPath
 */

/** The read-only face. Reactive through the getters. */
export const selection = {
  get paths() {
    return state.paths;
  },
  get focusPath() {
    return state.focusPath;
  },
  get anchorPath() {
    return state.anchorPath;
  },
};

/**
 * Where the focused photo sits in `view` right now.
 *
 * The fallback ladder is the one `restorePhotoSelection` already used for
 * Apple Photos reloads (modules/sidebar/applePhotosBrowsing.js): identity
 * first, then the first photo still selected, then the last position
 * clamped. That logic was right all along — it had only ever been wired to
 * one code path.
 *
 * @template {HasPath} T
 * @param {T[]} view
 * @returns {number}
 */
export function positionIn(view) {
  if (!view.length) return 0;
  const byFocus = state.focusPath ? view.findIndex((f) => f.path === state.focusPath) : -1;
  if (byFocus >= 0) return byFocus;
  const bySelection = view.findIndex((f) => state.paths.has(f.path));
  if (bySelection >= 0) return bySelection;
  return Math.max(0, Math.min(state.fallback, view.length - 1));
}

/**
 * Where the range anchor sits, falling back to the focus.
 * @template {HasPath} T
 * @param {T[]} view
 * @returns {number}
 */
export function anchorIn(view) {
  const i = state.anchorPath ? view.findIndex((f) => f.path === state.anchorPath) : -1;
  return i >= 0 ? i : positionIn(view);
}

/**
 * Move the focus to a position in `view` as it stands right now.
 *
 * The only way to move it. The position is resolved to a photo immediately
 * and the photo is what is kept, which is what makes an index from another
 * list impossible to express.
 * @template {HasPath} T
 * @param {T[]} view
 * @param {number} index
 */
export function focusAt(view, index) {
  state.fallback = index;
  state.focusPath = view[index]?.path ?? null;
}

/**
 * @template {HasPath} T
 * @param {T[]} view
 * @param {number} index
 */
export function selectOnly(view, index) {
  const frame = view[index];
  state.paths = new Set(frame ? [frame.path] : []);
  state.anchorPath = frame?.path ?? null;
}

/**
 * A click in the grid, with its modifiers: shift extends from the anchor,
 * cmd/ctrl toggles one photo, plain replaces the selection.
 * @template {HasPath} T
 * @param {T[]} view
 * @param {number} index
 * @param {MouseEvent} [event]
 */
export function selectGridItem(view, index, event) {
  const frame = view[index];
  if (!frame) return;

  if (event?.shiftKey) {
    const anchor = anchorIn(view);
    selectRange(view, anchor, index, Boolean(event.metaKey || event.ctrlKey));
  } else if (event?.metaKey || event?.ctrlKey) {
    toggle(frame.path);
    state.anchorPath = frame.path;
  } else {
    selectOnly(view, index);
  }
  focusAt(view, index);
}

/**
 * Select everything between two positions. `additive` keeps what was already
 * selected instead of replacing it.
 * @template {HasPath} T
 * @param {T[]} view
 * @param {number} a
 * @param {number} b
 * @param {boolean} [additive]
 */
export function selectRange(view, a, b, additive = false) {
  const from = Math.min(a, b);
  const to = Math.max(a, b);
  const next = additive ? new Set(state.paths) : new Set();
  for (let i = from; i <= to; i += 1) if (view[i]) next.add(view[i].path);
  state.paths = next;
}

/**
 * Add or remove one photo.
 * @param {string} path
 */
export function toggle(path) {
  const next = new Set(state.paths);
  if (next.has(path)) next.delete(path);
  else next.add(path);
  state.paths = next;
}

/**
 * @template {HasPath} T
 * @param {T[]} view
 */
export function selectAll(view) {
  state.paths = new Set(view.map((f) => f.path));
}

/**
 * Put the range anchor on a photo without touching the selection — after a
 * cmd-space toggle, so a following shift-arrow extends from there.
 * @param {string | null} path
 */
export function setAnchor(path) {
  state.anchorPath = path;
}

export function clearSelection() {
  state.paths = new Set();
}

/**
 * Replace the selection wholesale — for the Apple Photos reload, which
 * resolves its own identities and hands back the survivors.
 * @param {Set<string>} paths
 * @param {string | null} [anchorPath]
 */
export function setSelection(paths, anchorPath) {
  state.paths = paths;
  if (anchorPath !== undefined) state.anchorPath = anchorPath;
}

/**
 * The photos an action should apply to: everything selected, or the focused
 * one when nothing is.
 * @template {HasPath} T
 * @param {T[]} view
 * @returns {T[]}
 */
export function selectedFrames(view) {
  const selected = view.filter((frame) => state.paths.has(frame.path));
  if (selected.length) return selected;
  const focused = view[positionIn(view)];
  return focused ? [focused] : [];
}
