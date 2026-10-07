/**
 * The photos whose cell is on screen right now: the rows the viewport shows, no buffer.
 * The backend serves their thumbnails before anyone else's (see `thumb_queue.rs`).
 *
 * @param {{ path: string }[]} frames
 * @param {{ top: number, viewH: number, rowPitch: number, cols: number }} view
 * @returns {string[]}
 */
export function visiblePaths(frames, { top, viewH, rowPitch, cols }) {
  if (!(rowPitch > 0) || !(cols > 0) || !(viewH > 0)) return [];
  const first = Math.max(0, Math.floor(top / rowPitch));
  const last = Math.ceil((top + viewH) / rowPitch);
  return frames.slice(first * cols, (last + 1) * cols).map((f) => f.path);
}
