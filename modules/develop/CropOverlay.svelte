<script>
  /**
   * @typedef {Object} Props
   * @property {HTMLElement | null} [photoEl]
   * @property {any} [recipe]
   * @property {number | null} [renderAspect]
   * @property {string} [transformStr]
   * @property {((live: boolean) => void) | Function} [onCropChange]
   */

  /** @type {Props} */
  let {
    photoEl = null,
    recipe = null,
    renderAspect = null,
    transformStr = "",
    onCropChange = () => {},
  } = $props();

  /** @type {string | null} */
  let activeHandle = $state(null);
  let dragStartPos = { x: 0, y: 0 };
  let startCropBox = { x: 0, y: 0, w: 1, h: 1 };
  let cropBox = $state({ x: 0, y: 0, w: 1, h: 1 });
  /** @type {{ x: number, y: number, w: number, h: number } | null} */
  let photoBox = $state(null);

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
      x = Math.max(0, Math.min(1 - w, startCropBox.x + dx));
      y = Math.max(0, Math.min(1 - h, startCropBox.y + dy));
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

    cropBox = { x, y, w, h };
    if (recipe) {
      recipe.crop_x = Number(x.toFixed(4));
      recipe.crop_y = Number(y.toFixed(4));
      recipe.crop_w = Number(w.toFixed(4));
      recipe.crop_h = Number(h.toFixed(4));
    }
    onCropChange(true);
  }

  function onCropPointerUp() {
    if (activeHandle) {
      activeHandle = null;
      onCropChange(false);
    }
  }
</script>

{#if photoBox}
  <div
    class="crop-overlay-container"
    style="left: {photoBox.x}px; top: {photoBox.y}px; width: {photoBox.w}px; height: {photoBox.h}px; transform: {transformStr};"
    onpointermove={onCropPointerMove}
    onpointerup={onCropPointerUp}
    onpointercancel={onCropPointerUp}
    role="presentation"
  >
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
    position: absolute;
    z-index: 10;
    pointer-events: auto;
    overflow: hidden;
    touch-action: none;
  }
  .crop-rect {
    position: absolute;
    box-sizing: border-box;
    border: var(--stroke-width) solid rgba(255, 255, 255, 0.9);
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.55), 0 0 8px rgba(0, 0, 0, 0.5);
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
    height: var(--stroke-width);
  }
  .crop-grid-line.horizontal.at-third { top: 33.333%; }
  .crop-grid-line.horizontal.at-two-thirds { top: 66.666%; }
  .crop-grid-line.vertical {
    top: 0;
    bottom: 0;
    width: var(--stroke-width);
  }
  .crop-grid-line.vertical.at-third { left: 33.333%; }
  .crop-grid-line.vertical.at-two-thirds { left: 66.666%; }

  .crop-handle {
    position: absolute;
    box-sizing: border-box;
    background: #ffffff;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.6);
    z-index: 2;
    filter: drop-shadow(0 0 1px rgba(0, 0, 0, 0.9)) drop-shadow(0 0 1px rgba(0, 0, 0, 0.9));
  }
  .crop-handle::after {
    content: "";
    position: absolute;
    inset: -8px;
    cursor: inherit;
  }
  .crop-handle.handle-nw {
    top: -3px;
    left: -3px;
    width: 16px;
    height: 16px;
    background: transparent;
    border-top: 3px solid #ffffff;
    border-left: 3px solid #ffffff;
    cursor: nwse-resize;
  }
  .crop-handle.handle-ne {
    top: -3px;
    right: -3px;
    width: 16px;
    height: 16px;
    background: transparent;
    border-top: 3px solid #ffffff;
    border-right: 3px solid #ffffff;
    cursor: nesw-resize;
  }
  .crop-handle.handle-sw {
    bottom: -3px;
    left: -3px;
    width: 16px;
    height: 16px;
    background: transparent;
    border-bottom: 3px solid #ffffff;
    border-left: 3px solid #ffffff;
    cursor: nesw-resize;
  }
  .crop-handle.handle-se {
    bottom: -3px;
    right: -3px;
    width: 16px;
    height: 16px;
    background: transparent;
    border-bottom: 3px solid #ffffff;
    border-right: 3px solid #ffffff;
    cursor: nwse-resize;
  }

  .crop-handle.handle-n {
    top: -3px;
    left: 50%;
    transform: translateX(-50%);
    width: 24px;
    height: 4px;
    border-radius: var(--radius);
    cursor: ns-resize;
  }
  .crop-handle.handle-s {
    bottom: -3px;
    left: 50%;
    transform: translateX(-50%);
    width: 24px;
    height: 4px;
    border-radius: var(--radius);
    cursor: ns-resize;
  }
  .crop-handle.handle-w {
    left: -3px;
    top: 50%;
    transform: translateY(-50%);
    width: 4px;
    height: 24px;
    border-radius: var(--radius);
    cursor: ew-resize;
  }
  .crop-handle.handle-e {
    right: -3px;
    top: 50%;
    transform: translateY(-50%);
    width: 4px;
    height: 24px;
    border-radius: var(--radius);
    cursor: ew-resize;
  }
</style>
