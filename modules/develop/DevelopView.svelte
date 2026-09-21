<script>
  import Icon from "$lib/components/Icon.svelte";

  let {
    picked = null,
    imgUrl = "",
    useCanvas = false,
    showClipping = false,
    canvasEl = $bindable(null),
    imgFailed = $bindable(false),
    status = "",
    inflight = false,
    pendingPx = null,
    zoomMode = "frame",
    panning = false,
    developPhotoPercent = 90,
    recipe = null,
    renderAspect = null,
    trackDevelopViewport = () => {},
    showCropOverlay = false,
    onPhotoPointerDown = () => {},
    onPhotoPointerMove = () => {},
    onPhotoPointerUp = () => {},
  } = $props();

  let frameCap = $derived(
    zoomMode === "frame"
      ? `max-width: ${developPhotoPercent}%; max-height: ${developPhotoPercent}%;`
      : "",
  );

  let loaded = $state(false);
  /** @type {HTMLImageElement | null} */ let imgEl = $state(null);
  /** @type {HTMLCanvasElement | null} */ let clipCanvasEl = $state(null);
  let aspectRatio = $state("");

  let cropAspect = $derived(
    recipe?.crop_aspect && recipe.crop_aspect !== "original" && recipe.crop_aspect !== "free"
      ? recipe.crop_aspect.replace(":", "/")
      : aspectRatio
  );

  let objectFit = $derived(
    recipe?.crop_aspect && recipe.crop_aspect !== "original" && recipe.crop_aspect !== "free"
      ? "cover"
      : "contain"
  );

  let flipH = $derived(recipe?.flip_h ? -1 : 1);
  let flipV = $derived(recipe?.flip_v ? -1 : 1);
  let rotate = $derived(recipe?.crop_angle || 0);

  let transformStr = $derived(`rotate(${rotate}deg) scaleX(${flipH}) scaleY(${flipV})`);

  let matStyle = $derived(
    `${frameCap} ${cropAspect ? `aspect-ratio: ${cropAspect};` : ""} object-fit: ${objectFit}; transform: ${transformStr};`
  );

  $effect(() => {
    if (imgUrl) loaded = false;
  });

  $effect(() => {
    if (useCanvas && renderAspect) {
      aspectRatio = `${renderAspect}`;
    } else if (useCanvas && canvasEl?.width && canvasEl?.height) {
      aspectRatio = `${canvasEl.width} / ${canvasEl.height}`;
    }
  });

  /** @type {string | null} */
  let activeHandle = $state(null);
  let dragStartPos = { x: 0, y: 0 };
  let startCropBox = { x: 0, y: 0, w: 1, h: 1 };
  let cropBox = $state({ x: 0, y: 0, w: 1, h: 1 });

  let isCropping = $derived(
    showCropOverlay || (recipe?.crop_aspect && recipe.crop_aspect !== "original")
  );

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

  // The crop box's x/y/w/h are fractions of the DISPLAYED image, not real-
  // world units — so "1:1" isn't width===height in that space unless the
  // image itself happens to be square. This converts the chosen aspect
  // (e.g. 1:1, 16:9) into the w/h fraction that actually reads as that
  // ratio on screen, given the image's own aspect (renderAspect). Same
  // formula the preset-selection effect above already uses to seed the box;
  // this is what keeps it locked once you start dragging a handle.
  let lockedFractionalRatio = $derived.by(() => {
    const aspect = recipe?.crop_aspect;
    if (!aspect || aspect === "original" || aspect === "free") return null;
    const parts = aspect.split(":").map(Number);
    if (parts.length !== 2 || !(parts[0] > 0) || !(parts[1] > 0)) return null;
    const targetRatio = parts[0] / parts[1];
    const imgRatio = renderAspect || 1.5;
    return targetRatio / imgRatio; // w/h in crop-box fraction space
  });

  /**
   * Corner-drag resize that preserves `lockedFractionalRatio` — the other
   * three handle types (edge handles) are hidden in the markup whenever a
   * ratio is locked, since a single edge can't move without either
   * breaking the ratio or guessing which direction to compensate in.
   * @param {string} handle "nw" | "ne" | "sw" | "se"
   * @param {number} dx @param {number} dy @param {number} fr
   */
  function resizeCornerLocked(handle, dx, dy, fr) {
    const growRight = handle.includes("e");
    const growDown = handle.includes("s");
    const MIN = 0.05;

    // Follow whichever axis moved more; derive the other from the ratio —
    // avoids the two axes fighting over which one "wins" every frame.
    let w = growRight ? startCropBox.w + dx : startCropBox.w - dx;
    let h = growDown ? startCropBox.h + dy : startCropBox.h - dy;
    if (Math.abs(dx) >= Math.abs(dy)) {
      w = Math.max(MIN, w);
      h = w / fr;
    } else {
      h = Math.max(MIN, h);
      w = h * fr;
    }

    // The opposite corner is the fixed anchor.
    const anchorX = growRight ? startCropBox.x : startCropBox.x + startCropBox.w;
    const anchorY = growDown ? startCropBox.y : startCropBox.y + startCropBox.h;
    let x = growRight ? anchorX : anchorX - w;
    let y = growDown ? anchorY : anchorY - h;

    // Clamp to the frame, re-deriving the other side from `fr` each time so
    // hitting an edge shrinks the box instead of silently breaking the lock.
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
    const isCorner = activeHandle.length === 2; // "nw" | "ne" | "sw" | "se"

    if (activeHandle === "move") {
      x = Math.max(0, Math.min(1 - w, startCropBox.x + dx));
      y = Math.max(0, Math.min(1 - h, startCropBox.y + dy));
    } else if (fr && isCorner) {
      ({ x, y, w, h } = resizeCornerLocked(activeHandle, dx, dy, fr));
    } else if (fr) {
      // A locked ratio hides the edge handles in the markup — if one still
      // fires (e.g. a stale pointer capture), do nothing rather than
      // silently breaking the ratio.
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
  }

  /** @param {PointerEvent} e */
  function onCropPointerUp(e) {
    if (activeHandle) {
      activeHandle = null;
    }
  }

  $effect(() => {
    if (showClipping && clipCanvasEl) {
      renderClippingOverlay();
    }
  });

  /** @param {Event} e */
  function handleLoad(e) {
    imgFailed = false;
    loaded = true;
    const target = /** @type {HTMLImageElement} */ (e.currentTarget) || imgEl;
    if (target?.naturalWidth && target?.naturalHeight) {
      aspectRatio = `${target.naturalWidth} / ${target.naturalHeight}`;
    }
    if (showClipping) renderClippingOverlay();
  }

  function renderClippingOverlay() {
    if (!showClipping || !clipCanvasEl) return;
    let width = 0;
    let height = 0;
    let sourceData = null;

    if (useCanvas && canvasEl) {
      width = canvasEl.width;
      height = canvasEl.height;
      if (width <= 0 || height <= 0) return;
      const ctx = canvasEl.getContext("2d");
      if (ctx) {
        try { sourceData = ctx.getImageData(0, 0, width, height); } catch (e) {}
      }
    } else if (imgEl && loaded) {
      width = imgEl.naturalWidth || imgEl.width;
      height = imgEl.naturalHeight || imgEl.height;
      if (width <= 0 || height <= 0) return;
      const tmp = document.createElement("canvas");
      tmp.width = width;
      tmp.height = height;
      const tmpCtx = tmp.getContext("2d");
      if (tmpCtx) {
        try {
          tmpCtx.drawImage(imgEl, 0, 0);
          sourceData = tmpCtx.getImageData(0, 0, width, height);
        } catch (e) {}
      }
    }

    if (!sourceData || width <= 0 || height <= 0) return;
    clipCanvasEl.width = width;
    clipCanvasEl.height = height;
    const clipCtx = clipCanvasEl.getContext("2d");
    if (!clipCtx) return;

    const data = sourceData.data;
    const out = clipCtx.createImageData(width, height);
    const outData = out.data;

    for (let i = 0; i < data.length; i += 4) {
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      const a = data[i + 3];

      if (a < 10) continue;

      // Clipped whites (highlights > 252) -> vivid red
      if (r >= 252 && g >= 252 && b >= 252) {
        outData[i] = 255;
        outData[i + 1] = 30;
        outData[i + 2] = 30;
        outData[i + 3] = 230;
      }
      // Clipped blacks (shadows < 3) -> vivid blue
      else if (r <= 3 && g <= 3 && b <= 3) {
        outData[i] = 0;
        outData[i + 1] = 120;
        outData[i + 2] = 255;
        outData[i + 3] = 230;
      }
    }
    clipCtx.putImageData(out, 0, 0);
  }
</script>

<main
  use:trackDevelopViewport
  class="zoom-{zoomMode}"
  class:panning
  onpointerdown={onPhotoPointerDown}
  onpointermove={onPhotoPointerMove}
  onpointerup={onPhotoPointerUp}
  onpointercancel={onPhotoPointerUp}
  oncontextmenu={(e) => e.preventDefault()}
>
  {#if inflight || pendingPx !== null || status || (!loaded && imgUrl && !imgFailed && !useCanvas)}
    <div class="render-badge">
      <span class="render-spinner"></span>
      <span class="din render-label">{status || "Rendu en cours…"}</span>
    </div>
  {/if}

  {#if imgFailed}
    <div class="photo-mat photo-fallback">
      <Icon name="image-broken" size="44px" />
      <span class="fallback-name">{picked}</span>
      <span class="fallback-hint">Aperçu indisponible</span>
    </div>
  {:else if useCanvas}
    <canvas
      bind:this={canvasEl}
      class="photo-mat"
      class:dimmed={!!status}
      style={matStyle}
      aria-label={picked}
    ></canvas>
  {:else if imgUrl}
    <img
      bind:this={imgEl}
      class="photo-mat"
      class:dimmed={!!status}
      style={matStyle}
      data-no-zoom
      draggable="false"
      src={imgUrl}
      alt={picked}
      onerror={() => (imgFailed = true)}
      onload={handleLoad}
    />
  {:else}
    <p class="status">{status}</p>
  {/if}

  {#if showClipping && !imgFailed}
    <canvas
      bind:this={clipCanvasEl}
      class="photo-mat clip-overlay"
      style={matStyle}
    ></canvas>
    <div class="clip-legend">
      <span class="legend-item red">● Blancs écrasés</span>
      <span class="legend-item blue">● Noirs bouchés</span>
    </div>
  {/if}

  {#if isCropping && !imgFailed}
    <div
      class="crop-overlay-container photo-mat"
      style={matStyle}
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
        <div class="crop-grid-line h h1"></div>
        <div class="crop-grid-line h h2"></div>
        <div class="crop-grid-line v v1"></div>
        <div class="crop-grid-line v v2"></div>

        <!-- Corner Handles -->
        <div class="crop-handle handle-nw" onpointerdown={(e) => startCropResize('nw', e)} role="presentation"></div>
        <div class="crop-handle handle-ne" onpointerdown={(e) => startCropResize('ne', e)} role="presentation"></div>
        <div class="crop-handle handle-sw" onpointerdown={(e) => startCropResize('sw', e)} role="presentation"></div>
        <div class="crop-handle handle-se" onpointerdown={(e) => startCropResize('se', e)} role="presentation"></div>

        <!-- Edge handles only make sense in Libre — with a ratio locked,
             one edge alone can't move without either breaking the lock or
             guessing which way to compensate, so they're hidden rather
             than left to silently distort the crop. -->
        {#if !lockedFractionalRatio}
          <div class="crop-handle handle-n" onpointerdown={(e) => startCropResize('n', e)} role="presentation"></div>
          <div class="crop-handle handle-e" onpointerdown={(e) => startCropResize('e', e)} role="presentation"></div>
          <div class="crop-handle handle-s" onpointerdown={(e) => startCropResize('s', e)} role="presentation"></div>
          <div class="crop-handle handle-w" onpointerdown={(e) => startCropResize('w', e)} role="presentation"></div>
        {/if}
      </div>
    </div>
  {/if}
</main>

<style>
  main {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    overflow: hidden;
    background: var(--color-background, #121212);
  }
  .photo-mat {
    display: block;
    box-sizing: border-box;
    max-width: 90%;
    max-height: 90%;
    width: auto;
    height: auto;
    object-fit: contain;
    border: 12px solid var(--color-surface-high, #1e1e1e);
    background: transparent;
    border-radius: 18px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    transition: opacity var(--duration-instant);
    -webkit-user-drag: none;
    -webkit-user-select: none;
    user-select: none;
  }
  @media (prefers-color-scheme: dark) {
    .photo-mat {
      border: none;
      border-radius: var(--radius);
      box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
    }
  }
  :global([data-color-mode="dark"]) .photo-mat {
    border: none;
    border-radius: var(--radius);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
  }
  .photo-mat.dimmed {
    opacity: 0.75;
  }

  .clip-overlay {
    position: absolute;
    pointer-events: none;
    z-index: 5;
  }

  .crop-overlay-container {
    position: absolute;
    inset: 0;
    margin: auto;
    z-index: 10;
    pointer-events: auto;
    overflow: hidden;
    touch-action: none;
    border-color: transparent !important;
    box-shadow: none !important;
    background: transparent !important;
  }
  .crop-rect {
    position: absolute;
    box-sizing: border-box;
    border: 1px solid rgba(255, 255, 255, 0.9);
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.55), 0 0 8px rgba(0, 0, 0, 0.5);
    cursor: move;
  }
  .crop-grid-line {
    position: absolute;
    background: rgba(255, 255, 255, 0.35);
    pointer-events: none;
  }
  .crop-grid-line.h {
    left: 0;
    right: 0;
    height: 1px;
  }
  .crop-grid-line.h1 { top: 33.333%; }
  .crop-grid-line.h2 { top: 66.666%; }
  .crop-grid-line.v {
    top: 0;
    bottom: 0;
    width: 1px;
  }
  .crop-grid-line.v1 { left: 33.333%; }
  .crop-grid-line.v2 { left: 66.666%; }

  .crop-handle {
    position: absolute;
    box-sizing: border-box;
    background: #ffffff;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.6);
    z-index: 2;
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
    border-radius: var(--radius-sm);
    cursor: ns-resize;
  }
  .crop-handle.handle-s {
    bottom: -3px;
    left: 50%;
    transform: translateX(-50%);
    width: 24px;
    height: 4px;
    border-radius: var(--radius-sm);
    cursor: ns-resize;
  }
  .crop-handle.handle-w {
    left: -3px;
    top: 50%;
    transform: translateY(-50%);
    width: 4px;
    height: 24px;
    border-radius: var(--radius-sm);
    cursor: ew-resize;
  }
  .crop-handle.handle-e {
    right: -3px;
    top: 50%;
    transform: translateY(-50%);
    width: 4px;
    height: 24px;
    border-radius: var(--radius-sm);
    cursor: ew-resize;
  }

  .clip-legend {
    position: absolute;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 10;
    display: flex;
    gap: 12px;
    padding: 6px 14px;
    background: rgba(0, 0, 0, 0.85);
    backdrop-filter: blur(10px);
    border-radius: 999px;
    font-size: 0.72rem;
    font-family: var(--font-header, sans-serif);
    letter-spacing: 0.05em;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }
  .legend-item.red {
    color: #ff453a;
  }
  .legend-item.blue {
    color: #0a84ff;
  }

  .photo-fallback {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    min-width: 14rem;
    min-height: 10rem;
    color: var(--color-muted);
  }
  .fallback-name {
    font-family: var(--font-monospace, monospace);
    font-size: 0.8rem;
  }
  .fallback-hint {
    font-size: 0.68rem;
    letter-spacing: 0.05em;
    opacity: 0.7;
  }

  main.zoom-fill .photo-mat {
    max-width: 100%;
    max-height: 100%;
  }

  main.zoom-actual {
    overflow: auto;
    cursor: grab;
  }
  main.zoom-actual.panning {
    cursor: grabbing;
  }
  main.zoom-actual .photo-mat {
    max-width: none;
    max-height: none;
    padding: 0;
    background: transparent;
    box-shadow: none;
    border-radius: 0;
    object-fit: none;
  }

  .render-badge {
    position: absolute;
    top: 20px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    background: rgba(0, 0, 0, 0.75);
    backdrop-filter: blur(10px);
    border-radius: 999px;
    color: #fff;
    font-size: 0.75rem;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }
  .render-spinner {
    width: 10px;
    height: 10px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .status {
    color: var(--color-muted);
    font-size: 0.8rem;
    margin: 0;
  }
</style>
