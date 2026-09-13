/** @typedef {{path: string}} PhotoFrame */

/** @param {string | null | undefined} path */
export function photoIdentity(path) {
  return path?.startsWith("apple-photos://") ? path.slice("apple-photos://".length).split("/")[0] : path;
}

/**
 * Keep enough pages to retain selection after reordering, not just the first
 * 200 frames. Asset IDs survive filename changes. A stale request never commits.
 * @template {PhotoFrame} T
 * @param {(offset: number) => Promise<{frames: T[], total: number, next: number}>} readPage
 * @param {{minimumCount?: number, retainedPaths?: (string | null | undefined)[], isCurrent?: () => boolean}} options
 * @returns {Promise<{frames: T[], total: number, next: number} | null>}
 */
export async function reloadPhotoPages(readPage, { minimumCount = 0, retainedPaths = [], isCurrent = () => true } = {}) {
  const missing = new Set(retainedPaths.map(photoIdentity).filter(Boolean));
  /** @type {Map<string | null | undefined, T>} */
  const frames = new Map();
  let offset = 0;
  while (isCurrent()) {
    const page = await readPage(offset);
    if (!isCurrent()) return null;
    for (const frame of page.frames) {
      const id = photoIdentity(frame.path);
      frames.set(id, frame);
      missing.delete(id);
    }
    if (page.next >= page.total || (frames.size >= minimumCount && !missing.size)) {
      return { frames: [...frames.values()], total: page.total, next: page.next };
    }
    if (page.next <= offset) throw new Error("Apple Photos returned a page that did not advance.");
    offset = page.next;
  }
  return null;
}

/**
 * Reconcile focused/multi-selected rows against the refreshed, filtered view.
 * Deleted/filtered assets drop out; empty selection stays empty.
 * @param {PhotoFrame[]} rows
 * @param {{selected: Set<string>, focus?: string, anchor?: string, index: number}} previous
 */
export function restorePhotoSelection(rows, previous) {
  const ids = new Set([...previous.selected].map(photoIdentity));
  const selected = new Set(rows.filter((row) => ids.has(photoIdentity(row.path))).map((row) => row.path));
  let index = rows.findIndex((row) => photoIdentity(row.path) === photoIdentity(previous.focus));
  if (index < 0) index = rows.findIndex((row) => selected.has(row.path));
  if (index < 0) index = Math.max(0, Math.min(previous.index, rows.length - 1));
  if (previous.selected.size && !selected.size && rows[index]) selected.add(rows[index].path);
  const anchor = rows.findIndex((row) => photoIdentity(row.path) === photoIdentity(previous.anchor));
  return { selected, index, anchor: anchor < 0 ? index : anchor };
}
