/**
 * Surface and window interaction controller.
 *
 * Encapsulates pan gestures, zoom cycling, window presence tracking,
 * and view mode toggles.
 */

export const ZOOM_CYCLE = /** @type {const} */ (["frame", "fill", "actual"]);

/**
 * Cycle through zoom modes.
 * @param {"frame" | "fill" | "actual"} current
 * @param {boolean} [reverse]
 * @returns {"frame" | "fill" | "actual"}
 */
export function nextZoomMode(current, reverse = false) {
  const i = ZOOM_CYCLE.indexOf(current);
  if (i === -1) return "frame";
  const step = reverse ? -1 : 1;
  return ZOOM_CYCLE[(i + step + ZOOM_CYCLE.length) % ZOOM_CYCLE.length];
}

/**
 * Manages pointer presence events for focus-follows-mouse behavior.
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   onPresenceChange?: (inside: boolean) => void,
 * }} opts
 * @returns {() => void} cleanup function
 */
export function setupWindowPresence({ invoke, onPresenceChange = () => {} }) {
  if (typeof window === "undefined") return () => {};

  let pointerInside = false;

  /** @param {boolean} inside */
  const setPresence = (inside) => {
    pointerInside = inside;
    onPresenceChange(inside);
    invoke("set_focus_window_presence", { windowId: "main", inside }).catch(() => {});
  };

  const onFocus = () => {
    if (pointerInside) setPresence(true);
  };
  const onBlur = () => {
    pointerInside = false;
    setPresence(false);
  };
  const onPointerEnter = () => setPresence(true);
  const onPointerLeave = () => {
    pointerInside = false;
    setPresence(false);
  };
  const onPointerMove = () => {
    if (!pointerInside) setPresence(true);
  };

  /** @param {DragEvent} e */
  const preventDragOver = (e) => e.preventDefault();
  /** @param {DragEvent} e */
  const preventDrop = (e) => e.preventDefault();

  window.addEventListener("pointerenter", onPointerEnter);
  window.addEventListener("pointerleave", onPointerLeave);
  window.addEventListener("pointermove", onPointerMove);
  document.addEventListener("mouseleave", onPointerLeave);
  window.addEventListener("focus", onFocus);
  window.addEventListener("blur", onBlur);
  window.addEventListener("dragover", preventDragOver, false);
  window.addEventListener("drop", preventDrop, false);

  return () => {
    window.removeEventListener("pointerenter", onPointerEnter);
    window.removeEventListener("pointerleave", onPointerLeave);
    window.removeEventListener("pointermove", onPointerMove);
    document.removeEventListener("mouseleave", onPointerLeave);
    window.removeEventListener("focus", onFocus);
    window.removeEventListener("blur", onBlur);
    window.removeEventListener("dragover", preventDragOver, false);
    window.removeEventListener("drop", preventDrop, false);
    setPresence(false);
  };
}

/**
 * Creates pan gesture handlers for loupe drag interactions.
 * @param {{
 *   getZoomMode: () => string,
 *   getPhotoPercent: () => number,
 *   onPanningChange?: (panning: boolean) => void,
 * }} opts
 */
export function createPanGesture({ getZoomMode, getPhotoPercent, onPanningChange = () => {} }) {
  let panning = false;
  let panOrigin = { x: 0, y: 0, left: 0, top: 0 };

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onPointerDown(e) {
    const zoomMode = getZoomMode();
    const percent = getPhotoPercent();
    const canPan = zoomMode === "actual" || (zoomMode === "frame" && percent > 100);
    if (!canPan || e.button !== 0) return;
    const el = e.currentTarget;
    panning = true;
    onPanningChange(true);
    panOrigin = { x: e.clientX, y: e.clientY, left: el.scrollLeft, top: el.scrollTop };
    el.setPointerCapture?.(e.pointerId);
    e.stopPropagation();
  }

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onPointerMove(e) {
    if (!panning) return;
    const el = e.currentTarget;
    el.scrollLeft = panOrigin.left - (e.clientX - panOrigin.x);
    el.scrollTop = panOrigin.top - (e.clientY - panOrigin.y);
  }

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onPointerUp(e) {
    if (!panning) return;
    panning = false;
    onPanningChange(false);
    e.currentTarget.releasePointerCapture?.(e.pointerId);
  }

  return {
    isPanning: () => panning,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPhotoPointerDown: onPointerDown,
    onPhotoPointerMove: onPointerMove,
    onPhotoPointerUp: onPointerUp,
  };
}
