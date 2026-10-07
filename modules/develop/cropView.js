/**
 * The Crop tab's view, Lightroom style: the crop frame stays centred on screen and as large
 * as the stage allows, and it is the PICTURE that moves behind it.
 *
 * Nothing about the crop itself changes (it is still x/y/w/h in the photo, 0..1). What
 * changes is how it is shown: one transform, `translate() scale()`, put on the photo and
 * on the overlay together, chosen so that the centre of the crop lands on the centre of
 * the stage and the crop fills it.
 *
 * Geometry. The photo element is laid out centred in the stage, and CSS transforms act
 * around the element's own centre P. A point q of the photo (layout px, from P) is drawn at
 *     P + t + s · R · F · q
 * where F flips (scaleX/scaleY), R rotates (the straighten angle) and s, t are what this
 * function returns.
 *
 * The straighten angle turns the PICTURE under the frame, not the frame: the crop rectangle
 * is upright in the turned picture (that is what the engine cuts out), so the frame is drawn
 * through F alone — P + t + s · F · q — and R acts on the picture only. Putting the crop's
 * centre q_c on the stage's centre S therefore does not involve the angle:
 *     t = S − P − s · F · q_c.
 *
 * @typedef {{ x: number, y: number, w: number, h: number }} Box
 * @typedef {{ s: number, tx: number, ty: number }} View
 */

/** How much of the stage the crop may fill: the frame keeps some air around it. */
export const CROP_STAGE_FILL = 0.88;
/** A one-pixel-wide crop must not ask for a zoom of a million. */
export const CROP_MAX_SCALE = 8;

/**
 * @param {{
 *   stage: { w: number, h: number },
 *   photo: Box,
 *   crop: Box,
 *   flipH?: number,
 *   flipV?: number,
 *   scale?: number | null,
 * }} o
 *   `photo` is the photo element's layout box in the stage (offsetLeft/Top/Width/Height);
 *   `crop` is normalised to the photo; `scale` pins the zoom (mid-drag) instead of fitting.
 * @returns {View | null} null while nothing is measured yet.
 */
export function cropView({ stage, photo, crop, flipH = 1, flipV = 1, scale = null }) {
  if (!(stage?.w > 0) || !(stage?.h > 0) || !(photo?.w > 0) || !(photo?.h > 0)) return null;
  if (!(crop?.w > 0) || !(crop?.h > 0)) return null;

  let s = scale;
  if (s == null) {
    s = Math.min(
      CROP_MAX_SCALE,
      Math.min((stage.w * CROP_STAGE_FILL) / (crop.w * photo.w), (stage.h * CROP_STAGE_FILL) / (crop.h * photo.h)),
    );
  }

  const qx = (crop.x + crop.w / 2 - 0.5) * photo.w;
  const qy = (crop.y + crop.h / 2 - 0.5) * photo.h;
  const rx = qx * flipH;
  const ry = qy * flipV;

  const px = photo.x + photo.w / 2;
  const py = photo.y + photo.h / 2;
  return { s, tx: stage.w / 2 - px - s * rx, ty: stage.h / 2 - py - s * ry };
}

/** @param {View} v */
export function viewTransform(v) {
  return `translate(${v.tx.toFixed(2)}px, ${v.ty.toFixed(2)}px) scale(${v.s.toFixed(5)})`;
}

/**
 * Dragging the PICTURE by (dx, dy) — fractions of the photo's on-screen size — moves the
 * crop the other way, kept inside the photo.
 * @param {Box} start
 * @param {number} dx
 * @param {number} dy
 * @returns {Box}
 */
export function dragPicture(start, dx, dy) {
  return {
    x: Math.max(0, Math.min(1 - start.w, start.x - dx)),
    y: Math.max(0, Math.min(1 - start.h, start.y - dy)),
    w: start.w,
    h: start.h,
  };
}
