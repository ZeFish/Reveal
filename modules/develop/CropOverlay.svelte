<script>
  import { untrack } from "svelte";
  import { cropView, viewTransform as transformOf, dragPicture } from "./cropView.js";
  import { fitInsideTurned } from "./cropFit.js";

  /**
   * @typedef {Object} Props
   * @property {HTMLElement | null} [photoEl]
   * @property {any} [recipe]
   * @property {number | null} [renderAspect]
   * @property {string} [transformStr]
   * @property {((live: boolean) => void) | Function} [onCropChange]
   * @property {string} [viewTransform] out: the zoom/pan that centres the crop frame — the photo wears it too
   * @property {string} [viewTransition] out: how the photo should animate into that view
   */

  /** @type {Props} */
  let {
    photoEl = null,
    recipe = null,
    renderAspect = null,
    transformStr = "",
    onCropChange = () => {},
    viewTransform = $bindable(""),
    viewTransition = $bindable(""),
  } = $props();

  /** @type {string | null} */
  let activeHandle = $state(null);
  let dragStartPos = { x: 0, y: 0 };
  let startCropBox = { x: 0, y: 0, w: 1, h: 1 };
  let cropBox = $state({ x: 0, y: 0, w: 1, h: 1 });
  /** @type {{ x: number, y: number, w: number, h: number } | null} */
  let photoBox = $state(null);
  // The stage the photo sits centred in; the frame is fitted to it.
  let stage = $state({ w: 0, h: 0 });
  // The view held while a handle is being dragged (the frame resizes over a still picture)
  // or whose zoom is held while the picture is dragged (it pans, the frame stays).
  /** @type {import("./cropView.js").View | null} */
  let frozen = $state(null);

  // Measure the photo element's layout box (unaffected by rotate/flip) so the overlay
  // aligns with pixel precision even when the photo is constrained.
  $effect(() => {
    const el = photoEl;
    if (!el) {
      photoBox = null;
      return;
    }
    const measure = () => {
      photoBox = { x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight };
      const parent = el.parentElement;
      if (parent) stage = { w: parent.clientWidth, h: parent.clientHeight };
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    if (el.parentElement) observer.observe(el.parentElement);
    return () => observer.disconnect();
  });

  $effect(() => {
    const aspect = recipe?.crop_aspect;
    if (aspect === "original" || aspect === "free" || !aspect) {
      if (!activeHandle) {
        cropBox = {
          x: recipe?.crop_x ?? 0,
          y: recipe?.crop_y ?? 0,
          w: recipe?.crop_w ?? 1,
          h: recipe?.crop_h ?? 1,
        };
      }
    } else {
      const parts = aspect.split(":").map(Number);
      if (parts.length === 2 && parts[0] > 0 && parts[1] > 0) {
        const targetRatio = parts[0] / parts[1];
        const imgRatio = renderAspect || 1.5;
        if (!activeHandle) {
          if (targetRatio < imgRatio) {
            const w = targetRatio / imgRatio;
            const h = 1.0;
            cropBox = { x: (1 - w) / 2, y: 0, w, h };
          } else {
            const w = 1.0;
            const h = imgRatio / targetRatio;
            cropBox = { x: 0, y: (1 - h) / 2, w, h };
          }
        }
      }
    }
  });

  /** Write a crop frame to the recipe, rounded the way the pointer path does. */
  function commitBox(/** @type {{ x: number, y: number, w: number, h: number }} */ box) {
    cropBox = box;
    if (!recipe) return;
    recipe.crop_x = Number(box.x.toFixed(4));
    recipe.crop_y = Number(box.y.toFixed(4));
    recipe.crop_w = Number(box.w.toFixed(4));
    recipe.crop_h = Number(box.h.toFixed(4));
  }

  // The picture turns under an upright frame, so the frame must stay inside the turned
  // picture (see cropFit.js). Turning the slider, or changing the proportions, slides it back
  // in and shrinks it only if it can no longer fit.
  $effect(() => {
    const angle = recipe?.crop_angle || 0;
    void recipe?.crop_aspect;
    const aspect = renderAspect;
    if (!angle || !aspect || activeHandle) return;
    untrack(() => {
      const fitted = fitInsideTurned(cropBox, angle, aspect);
      const moved =
        Math.abs(fitted.x - cropBox.x) > 1e-4 ||
        Math.abs(fitted.y - cropBox.y) > 1e-4 ||
        Math.abs(fitted.w - cropBox.w) > 1e-4 ||
        Math.abs(fitted.h - cropBox.h) > 1e-4;
      if (!moved) return;
      commitBox(fitted);
      onCropChange(true);
    });
  });

  /** @param {number | null} [scale] */
  function viewFor(scale = null) {
    if (!photoBox) return null;
    return cropView({
      stage,
      photo: photoBox,
      crop: cropBox,
      flipH: recipe?.flip_h ? -1 : 1,
      flipV: recipe?.flip_v ? -1 : 1,
      scale,
    });
  }

  // Lightroom's crop tool: the frame stays centred and as big as the stage allows; the PICTURE
  // moves behind it. Dragging the picture follows the pointer with the zoom held (the frame
  // never leaves the middle); dragging a handle changes the frame over a still picture, and
  // the view re-fits, animated, when it is let go.
  const view = $derived.by(() => {
    if (activeHandle === "move" && frozen) return viewFor(frozen.s);
    if (activeHandle && frozen) return frozen;
    return viewFor();
  });

  $effect(() => {
    viewTransform = view ? transformOf(view) : "";
    viewTransition = activeHandle ? "none" : "transform 260ms cubic-bezier(0.22, 0.8, 0.2, 1)";
  });

  // Fractional w/h ratio in display space corresponding to the chosen aspect
  let lockedFractionalRatio = $derived.by(() => {
    const aspect = recipe?.crop_aspect;
    if (!aspect || aspect === "original" || aspect === "free") return null;
    const parts = aspect.split(":").map(Number);
    if (parts.length !== 2 || !(parts[0] > 0) || !(parts[1] > 0)) return null;
    const targetRatio = parts[0] / parts[1];
    const imgRatio = renderAspect || 1.5;
    return targetRatio / imgRatio;
  });

  /**
   * Corner-drag resize that preserves lockedFractionalRatio.
   * @param {string} handle "nw" | "ne" | "sw" | "se"
   * @param {number} dx @param {number} dy @param {number} fr
   */
  function resizeCornerLocked(handle, dx, dy, fr) {
    const growRight = handle.includes("e");
    const growDown = handle.includes("s");
    const MIN = 0.05;

    let w = growRight ? startCropBox.w + dx : startCropBox.w - dx;
    let h = growDown ? startCropBox.h + dy : startCropBox.h - dy;
    if (Math.abs(dx) >= Math.abs(dy)) {
      w = Math.max(MIN, w);
      h = w / fr;
    } else {
      h = Math.max(MIN, h);
      w = h * fr;
    }

    const anchorX = growRight ? startCropBox.x : startCropBox.x + startCropBox.w;
    const anchorY = growDown ? startCropBox.y : startCropBox.y + startCropBox.h;
    let x = growRight ? anchorX : anchorX - w;
    let y = growDown ? anchorY : anchorY - h;

    if (x < 0) { w += x; x = 0; h = w / fr; }
    if (y < 0) { h += y; y = 0; w = h * fr; }
    if (x + w > 1) { w = 1 - x; h = w / fr; }
    if (y + h > 1) { h = 1 - y; w = h * fr; }

    return { x, y, w, h };
  }

  /**
   * @param {string} handle
   * @param {PointerEvent & { currentTarget: HTMLElement }} e
   */
  function startCropResize(handle, e) {
    e.preventDefault();
    e.stopPropagation();
    frozen = viewFor();
    activeHandle = handle;
    dragStartPos = { x: e.clientX, y: e.clientY };
    startCropBox = { ...cropBox };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function startCropMove(e) {
    if (e.target !== e.currentTarget) return;
    e.preventDefault();
    e.stopPropagation();
    frozen = viewFor();
    activeHandle = "move";
    dragStartPos = { x: e.clientX, y: e.clientY };
    startCropBox = { ...cropBox };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onCropPointerMove(e) {
    if (!activeHandle) return;
    const overlay = e.currentTarget;
    const rect = overlay.getBoundingClientRect();
    if (!rect.width || !rect.height) return;

    const dx = (e.clientX - dragStartPos.x) / rect.width;
    const dy = (e.clientY - dragStartPos.y) / rect.height;

    let { x, y, w, h } = startCropBox;

    const fr = lockedFractionalRatio;
    const isCorner = activeHandle.length === 2;

    if (activeHandle === "move") {
      // Dragging the picture: the crop moves the other way.
      ({ x, y } = dragPicture(startCropBox, dx, dy));
    } else if (fr && isCorner) {
      ({ x, y, w, h } = resizeCornerLocked(activeHandle, dx, dy, fr));
    } else if (fr) {
      return;
    } else {
      let newX = x;
      let newY = y;
      let newW = w;
      let newH = h;

      if (activeHandle.includes("e")) newW = Math.max(0.1, Math.min(1 - startCropBox.x, startCropBox.w + dx));
      if (activeHandle.includes("s")) newH = Math.max(0.1, Math.min(1 - startCropBox.y, startCropBox.h + dy));
      if (activeHandle.includes("w")) {
        const maxDx = startCropBox.w - 0.1;
        const actualDx = Math.max(-startCropBox.x, Math.min(maxDx, dx));
        newX = startCropBox.x + actualDx;
        newW = startCropBox.w - actualDx;
      }
      if (activeHandle.includes("n")) {
        const maxDy = startCropBox.h - 0.1;
        const actualDy = Math.max(-startCropBox.y, Math.min(maxDy, dy));
        newY = startCropBox.y + actualDy;
        newH = startCropBox.h - actualDy;
      }

      x = newX;
      y = newY;
      w = newW;
      h = newH;
    }

    let next = { x, y, w, h };
    const angle = recipe?.crop_angle || 0;
    if (angle && renderAspect) next = fitInsideTurned(next, angle, renderAspect);
    commitBox(next);
    onCropChange(true);
  }

  function onCropPointerUp() {
    if (activeHandle) {
      activeHandle = null;
      frozen = null;
      onCropChange(false);
    }
  }
</script>

{#if photoBox}
  <div
    class="crop-overlay-container"
    style="left: {photoBox.x}px; top: {photoBox.y}px; width: {photoBox.w}px; height: {photoBox.h}px; --s: {view?.s ?? 1}; transform: {viewTransform} {transformStr}; transition: {viewTransition || 'none'};"
    onpointermove={onCropPointerMove}
    onpointerup={onCropPointerUp}
    onpointercancel={onCropPointerUp}
    role="presentation"
  >
    <!-- The dimming of everything outside the crop. It is the 9999px shadow of a
         hole the size of the crop, so it has to be clipped to the photo — and that
         clip used to be the container's, which also cut the handles that stick
         out of a crop touching the photo's edge. The clip lives on this layer only. -->
    <div class="crop-mask" aria-hidden="true">
      <div
        class="crop-hole"
        style="
          left: {cropBox.x * 100}%;
          top: {cropBox.y * 100}%;
          width: {cropBox.w * 100}%;
          height: {cropBox.h * 100}%;
        "
      ></div>
    </div>
    <div
      class="crop-rect"
      style="
        left: {cropBox.x * 100}%;
        top: {cropBox.y * 100}%;
        width: {cropBox.w * 100}%;
        height: {cropBox.h * 100}%;
      "
      onpointerdown={startCropMove}
      role="presentation"
    >
      <!-- Rule of Thirds Grid Lines -->
      <div class="crop-grid-line horizontal at-third"></div>
      <div class="crop-grid-line horizontal at-two-thirds"></div>
      <div class="crop-grid-line vertical at-third"></div>
      <div class="crop-grid-line vertical at-two-thirds"></div>

      <!-- Corner Handles -->
      <div class="crop-handle handle-nw" onpointerdown={(e) => startCropResize('nw', e)} role="presentation"></div>
      <div class="crop-handle handle-ne" onpointerdown={(e) => startCropResize('ne', e)} role="presentation"></div>
      <div class="crop-handle handle-sw" onpointerdown={(e) => startCropResize('sw', e)} role="presentation"></div>
      <div class="crop-handle handle-se" onpointerdown={(e) => startCropResize('se', e)} role="presentation"></div>

      <!-- Edge handles (Libre / unlocked aspect only) -->
      {#if !lockedFractionalRatio}
        <div class="crop-handle handle-n" onpointerdown={(e) => startCropResize('n', e)} role="presentation"></div>
        <div class="crop-handle handle-e" onpointerdown={(e) => startCropResize('e', e)} role="presentation"></div>
        <div class="crop-handle handle-s" onpointerdown={(e) => startCropResize('s', e)} role="presentation"></div>
        <div class="crop-handle handle-w" onpointerdown={(e) => startCropResize('w', e)} role="presentation"></div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .crop-overlay-container {
    /* One screen pixel, whatever zoom the view is at: the frame's lines and handles are
       drawn in the zoomed layer, so they are sized against it. */
    --u: calc(1px / var(--s, 1));
    position: absolute;
    z-index: 10;
    pointer-events: auto;
    /* No overflow clip here: the handles straddle the crop's edge, and at the
       photo's edge half of each would be cut. */
    touch-action: none;
  }
  .crop-mask {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }
  .crop-hole {
    position: absolute;
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.55);
  }
  .crop-rect {
    position: absolute;
    box-sizing: border-box;
    border: calc(var(--stroke-width) / var(--s, 1)) solid rgba(255, 255, 255, 0.9);
    box-shadow: 0 0 8px rgba(0, 0, 0, 0.5);
    cursor: move;
  }
  .crop-grid-line {
    position: absolute;
    background: rgba(255, 255, 255, 0.35);
    pointer-events: none;
  }
  .crop-grid-line.horizontal {
    left: 0;
    right: 0;
    height: calc(var(--stroke-width) / var(--s, 1));
  }
  .crop-grid-line.horizontal.at-third { top: 33.333%; }
  .crop-grid-line.horizontal.at-two-thirds { top: 66.666%; }
  .crop-grid-line.vertical {
    top: 0;
    bottom: 0;
    width: calc(var(--stroke-width) / var(--s, 1));
  }
  .crop-grid-line.vertical.at-third { left: 33.333%; }
  .crop-grid-line.vertical.at-two-thirds { left: 66.666%; }

  .crop-handle {
    position: absolute;
    box-sizing: border-box;
    background: #ffffff;
    box-shadow: 0 calc(1 * var(--u)) calc(4 * var(--u)) rgba(0, 0, 0, 0.6);
    z-index: 2;
    filter: drop-shadow(0 0 calc(1 * var(--u)) rgba(0, 0, 0, 0.9)) drop-shadow(0 0 calc(1 * var(--u)) rgba(0, 0, 0, 0.9));
  }
  .crop-handle::after {
    content: "";
    position: absolute;
    inset: calc(-8 * var(--u));
    cursor: inherit;
  }
  .crop-handle.handle-nw {
    top: calc(-3 * var(--u));
    left: calc(-3 * var(--u));
    width: calc(16 * var(--u));
    height: calc(16 * var(--u));
    background: transparent;
    border-top: calc(3 * var(--u)) solid #ffffff;
    border-left: calc(3 * var(--u)) solid #ffffff;
    cursor: nwse-resize;
  }
  .crop-handle.handle-ne {
    top: calc(-3 * var(--u));
    right: calc(-3 * var(--u));
    width: calc(16 * var(--u));
    height: calc(16 * var(--u));
    background: transparent;
    border-top: calc(3 * var(--u)) solid #ffffff;
    border-right: calc(3 * var(--u)) solid #ffffff;
    cursor: nesw-resize;
  }
  .crop-handle.handle-sw {
    bottom: calc(-3 * var(--u));
    left: calc(-3 * var(--u));
    width: calc(16 * var(--u));
    height: calc(16 * var(--u));
    background: transparent;
    border-bottom: calc(3 * var(--u)) solid #ffffff;
    border-left: calc(3 * var(--u)) solid #ffffff;
    cursor: nesw-resize;
  }
  .crop-handle.handle-se {
    bottom: calc(-3 * var(--u));
    right: calc(-3 * var(--u));
    width: calc(16 * var(--u));
    height: calc(16 * var(--u));
    background: transparent;
    border-bottom: calc(3 * var(--u)) solid #ffffff;
    border-right: calc(3 * var(--u)) solid #ffffff;
    cursor: nwse-resize;
  }

  .crop-handle.handle-n {
    top: calc(-3 * var(--u));
    left: 50%;
    transform: translateX(-50%);
    width: calc(24 * var(--u));
    height: calc(4 * var(--u));
    border-radius: var(--radius);
    cursor: ns-resize;
  }
  .crop-handle.handle-s {
    bottom: calc(-3 * var(--u));
    left: 50%;
    transform: translateX(-50%);
    width: calc(24 * var(--u));
    height: calc(4 * var(--u));
    border-radius: var(--radius);
    cursor: ns-resize;
  }
  .crop-handle.handle-w {
    left: calc(-3 * var(--u));
    top: 50%;
    transform: translateY(-50%);
    width: calc(4 * var(--u));
    height: calc(24 * var(--u));
    border-radius: var(--radius);
    cursor: ew-resize;
  }
  .crop-handle.handle-e {
    right: calc(-3 * var(--u));
    top: 50%;
    transform: translateY(-50%);
    width: calc(4 * var(--u));
    height: calc(24 * var(--u));
    border-radius: var(--radius);
    cursor: ew-resize;
  }
</style>
