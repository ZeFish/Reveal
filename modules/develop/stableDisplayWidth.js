/**
 * How wide the photo is drawn, held steady while its render resolution moves.
 *
 * A live render (a slider being dragged) is a small image and the settled one a
 * large one, and an `<img>` with `width: auto` is drawn at its natural size, capped
 * by the frame. So the picture shrank while a Spektra slider moved and sprang
 * back when the full render landed. The width kept here is the widest render
 * of the SAME shape seen for this photo; the view pins the image to it, and a
 * smaller live render is simply drawn into the same box.
 *
 * It resets when the shape changes (a crop, a rotation) — and the view resets it
 * when another photo is picked, so a thumbnail is never blown up to the size of
 * the render that will replace it.
 *
 * A develop render (a live or settled one, as opposed to the grid's thumbnail) stands for the
 * size the settled render will have. When only live renders have landed — someone moved a
 * slider the moment the photo opened — the settled one has not, and pinning to the live
 * render's own 768 px drew the photo small until it did.
 *
 * @param {{ key: string, w: number }} previous
 * @param {number} naturalWidth
 * @param {number} naturalHeight
 * @param {boolean} [rendered] this image is a develop render, not a thumbnail
 * @returns {{ key: string, w: number }}
 */
export function nextStableWidth(previous, naturalWidth, naturalHeight, rendered = false) {
  if (!(naturalWidth > 0) || !(naturalHeight > 0)) return previous;
  const aspect = naturalWidth / naturalHeight;
  const key = aspect.toFixed(3);
  const settledWidth = aspect >= 1 ? SETTLED_EDGE : SETTLED_EDGE * aspect;
  const w = rendered ? Math.max(naturalWidth, settledWidth) : naturalWidth;
  if (previous.key === key) return { key, w: Math.max(previous.w, w) };
  return { key, w };
}

/** The long edge of the render a photo settles at (`PREVIEW_PX` in developOperations.js). */
export const SETTLED_EDGE = 2048;

export const NO_STABLE_WIDTH = Object.freeze({ key: "", w: 0 });

/**
 * The box the photo is drawn in, worked out from the frame instead of from the
 * image in front of us: the settled width capped by the frame, and the height that
 * keeps the picture's shape. Sizing by `width` alone would break the portrait
 * case, where the HEIGHT is what the frame caps.
 *
 * Returns null when it does not apply (no measure yet, nothing settled), and the
 * caller falls back to the plain CSS caps.
 *
 * @param {{ frameW: number, frameH: number, percent: number, stable: { key: string, w: number } }} o
 * @returns {{ w: number, h: number } | null}
 */
export function displayBox({ frameW, frameH, percent, stable }) {
  const aspect = parseFloat(stable.key);
  if (!(frameW > 0) || !(frameH > 0) || !(percent > 0) || percent > 100) return null;
  if (!(stable.w > 0) || !(aspect > 0)) return null;
  const cap = percent / 100;
  const fitW = Math.min(frameW * cap, frameH * cap * aspect);
  const w = Math.min(stable.w, fitW);
  return { w, h: w / aspect };
}
