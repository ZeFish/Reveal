<script>
  import Icon from "$lib/components/Icon.svelte";

  let {
    picked = null,
    imgUrl = "",
    useCanvas = false,
    // Bumped by +page.svelte's pump() every time it paints fresh pixels
    // into canvasEl — canvasEl.width/height alone don't change between two
    // renders of the same photo at the same output size, so the histogram
    // effect below needs this as an explicit dependency or it goes stale
    // after the first Rapid-engine render.
    canvasVersion = 0,
    showClipping = false,
    caption = "",
    showCaption = false,
    canvasEl = $bindable(null),
    imgFailed = $bindable(false),
    // {r,g,b,luma: Uint32Array(256)} bin counts for the currently displayed
    // frame — recomputed alongside the clipping overlay, off the same pixel
    // read, so DevTab's histogram widget (this component's sibling, docked
    // or detached — see DevelopPanel.svelte's own docblock) always matches
    // what's actually on screen.
    histogram = $bindable(null),
    // Waveform + vectorscope for the same frame, off the same pixel read as
    // the histogram. Deliberately NOT relayed to a detached dev panel the
    // way `histogram` is (+page.svelte's sendDevStateToPanel) — the
    // histogram is 4×256 numbers, these are ~130k, which is fine as a plain
    // prop in the docked window's own JS but absurd through a JSON event on
    // every slider tick. Detached panel keeps the histogram modes only.
    scopes = $bindable(null),
    status = "",
    inflight = false,
    pendingPx = null,
    zoomMode = "frame",
    panning = false,
    developPhotoPercent = 90,
    recipe = null,
    renderAspect = null,
    showCropOverlay = false,
    onPhotoPointerDown = () => {},
    onPhotoPointerMove = () => {},
    onPhotoPointerUp = () => {},
  } = $props();

  // Minimal markdown-to-HTML for the caption overlay — just bold/italic, the
  // two the InfoBlock toolbar writes. Escapes first since this is
  // `{@html}`-rendered; the caption is the photographer's own text, but the
  // escape is cheap insurance against `<`/`>` in it rendering as markup.
  let captionHtml = $derived(
    caption
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/\*(.+?)\*/g, "<em>$1</em>")
  );

  let frameCap = $derived(
    zoomMode === "frame"
      ? `max-width: ${developPhotoPercent}%; max-height: ${developPhotoPercent}%;`
      : "",
  );

  // Now that the caches usually answer instantly, a render that finishes in
  // 30ms used to flash a full pill on screen — more distracting than the wait
  // it announced. So: nothing at all for the first BUSY_DELAY_MS, then a bare
  // spinner, and words only when there's an actual `status` to read.
  const BUSY_DELAY_MS = 400;
  let loaded = $state(false);
  let busy = $derived(
    inflight || pendingPx !== null || !!status || (!loaded && !!imgUrl && !imgFailed && !useCanvas)
  );
  let busyVisible = $state(false);
  $effect(() => {
    if (!busy) {
      busyVisible = false;
      return;
    }
    const t = setTimeout(() => (busyVisible = true), BUSY_DELAY_MS);
    return () => clearTimeout(t);
  });

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
  // Only true while the Crop tab is actually open — a persisted non-"original"
  // crop_aspect used to keep this (and the grid overlay it drives) on in
  // every other tab too, well after the crop itself was chosen.
  let isCropping = $derived(showCropOverlay);
  // crop_angle never reaches the Rust render pipeline (no such field on the
  // engine's Recipe) — it's a live preview aid for straightening WHILE
  // adjusting in Crop, not a real edit to keep showing afterward. Left
  // applied unconditionally, the whole rectangular photo stayed visibly
  // tilted inside its own frame in every other tab (Francis, reproduced
  // 2026-09-21: "toute la photo qui tourne" after leaving Crop).
  let rotate = $derived(isCropping ? (recipe?.crop_angle || 0) : 0);

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
    // Same staleness gap as the histogram effect below: in canvas (Rapid)
    // mode nothing else re-triggers this when a new frame lands at the same
    // output size, so canvasVersion has to be an explicit dependency.
    canvasVersion;
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
    updateHistogram();
  }

  $effect(() => {
    // useCanvas's own pixels are written by the caller (+page.svelte's pump())
    // straight into canvasEl, outside this component. canvasVersion is the
    // explicit signal that a fresh frame landed — canvasEl.width/height
    // alone don't change between two renders at the same output size (e.g.
    // nudging exposure), so keying only on those left the histogram frozen
    // after the first Rapid-engine render.
    canvasVersion;
    if (useCanvas && canvasEl?.width && canvasEl?.height) updateHistogram();
  });

  /** @returns {{width: number, height: number, data: ImageData} | null} */
  function getSourcePixelData() {
    if (useCanvas && canvasEl) {
      const width = canvasEl.width;
      const height = canvasEl.height;
      if (width <= 0 || height <= 0) return null;
      const ctx = canvasEl.getContext("2d");
      if (!ctx) return null;
      try {
        return { width, height, data: ctx.getImageData(0, 0, width, height) };
      } catch {
        return null;
      }
    } else if (imgEl && loaded) {
      const width = imgEl.naturalWidth || imgEl.width;
      const height = imgEl.naturalHeight || imgEl.height;
      if (width <= 0 || height <= 0) return null;
      const tmp = document.createElement("canvas");
      tmp.width = width;
      tmp.height = height;
      const tmpCtx = tmp.getContext("2d");
      if (!tmpCtx) return null;
      try {
        tmpCtx.drawImage(imgEl, 0, 0);
        return { width, height, data: tmpCtx.getImageData(0, 0, width, height) };
      } catch {
        return null;
      }
    }
    return null;
  }

  function renderClippingOverlay() {
    if (!showClipping || !clipCanvasEl) return;
    const source = getSourcePixelData();
    if (!source) return;
    const { width, height, data: sourceData } = source;
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

  // Waveform/vectorscope buffers, allocated once per view and refilled in
  // place — a fresh set every render would be ~600 KB of garbage per slider
  // tick. Instance-level (not module-level) on purpose: the fullscreen
  // viewer is a second DevelopView, and two of them sharing one buffer
  // would interleave their pixels into each other's scope.
  const WF_COLS = 256; // waveform horizontal resolution (one column per bin)
  const WF_LEVELS = 128; // waveform vertical (value) resolution
  const VEC_SIZE = 128; // vectorscope is square, in Cb/Cr
  const wfR = new Uint32Array(WF_COLS * WF_LEVELS);
  const wfG = new Uint32Array(WF_COLS * WF_LEVELS);
  const wfB = new Uint32Array(WF_COLS * WF_LEVELS);
  const wfL = new Uint32Array(WF_COLS * WF_LEVELS);
  const vec = new Uint32Array(VEC_SIZE * VEC_SIZE);

  function updateHistogram() {
    const source = getSourcePixelData();
    if (!source) return;
    const { width, height } = source;
    const data = source.data.data;
    const r = new Uint32Array(256);
    const g = new Uint32Array(256);
    const b = new Uint32Array(256);
    const luma = new Uint32Array(256);
    wfR.fill(0);
    wfG.fill(0);
    wfB.fill(0);
    wfL.fill(0);
    vec.fill(0);

    // Every pixel feeds the histogram — that's four increments, the cost it
    // always had. The waveform and vectorscope take every ROW_STRIDE-th row
    // instead: a waveform is read column by column, so dropping rows costs
    // nothing legible while keeping this whole pass near its original cost
    // even at full preview resolution.
    const ROW_STRIDE = 3;
    for (let y = 0; y < height; y++) {
      const sampleRow = y % ROW_STRIDE === 0;
      let i = y * width * 4;
      for (let x = 0; x < width; x++, i += 4) {
        if (data[i + 3] < 10) continue;
        const rv = data[i], gv = data[i + 1], bv = data[i + 2];
        r[rv]++;
        g[gv]++;
        b[bv]++;
        const lv = (0.2126 * rv + 0.7152 * gv + 0.0722 * bv) | 0;
        luma[lv]++;

        if (!sampleRow) continue;
        const base = (((x * WF_COLS) / width) | 0) * WF_LEVELS;
        wfR[base + ((rv * WF_LEVELS) >> 8)]++;
        wfG[base + ((gv * WF_LEVELS) >> 8)]++;
        wfB[base + ((bv * WF_LEVELS) >> 8)]++;
        wfL[base + ((lv * WF_LEVELS) >> 8)]++;

        // BT.709 chroma difference, the same axes a broadcast vectorscope
        // plots: Cb right, Cr up. Both land in -128..127 for 8-bit RGB.
        const cb = -0.1146 * rv - 0.3854 * gv + 0.5 * bv;
        const cr = 0.5 * rv - 0.4542 * gv - 0.0458 * bv;
        const vx = (((cb + 128) * VEC_SIZE) / 256) | 0;
        const vy = (((128 - cr) * VEC_SIZE) / 256) | 0;
        vec[vy * VEC_SIZE + vx]++;
      }
    }
    histogram = { r, g, b, luma };
    scopes = {
      cols: WF_COLS,
      levels: WF_LEVELS,
      vecSize: VEC_SIZE,
      r: wfR,
      g: wfG,
      b: wfB,
      luma: wfL,
      vector: vec,
    };
  }

  // Capture One-style loupe: press on the photo, a circle follows the
  // cursor showing that spot at native pixel scale (no upscaling blur —
  // imageSmoothingEnabled off), release to dismiss. Only in "frame" zoom
  // (fit-to-view) — "actual" already shows 1:1 pixels and owns pointer-down
  // for panning (see the prop handlers above), and crop owns its own
  // overlay's pointer events entirely.
  let loupeActive = $state(false);
  let loupePos = $state({ x: 0, y: 0 }); // client coords, for the fixed overlay
  /** @type {HTMLCanvasElement | null} */
  let loupeCanvasEl = $state(null);
  const LOUPE_DIAMETER = 220;
  const LOUPE_ZOOM = 2; // 2x native pixels — plain 1x reads as "too tight" on a retina display

  function loupeSource() {
    if (useCanvas && canvasEl?.width) return { el: canvasEl, w: canvasEl.width, h: canvasEl.height };
    if (imgEl && loaded) {
      const w = imgEl.naturalWidth || imgEl.width;
      const h = imgEl.naturalHeight || imgEl.height;
      if (w && h) return { el: imgEl, w, h };
    }
    return null;
  }

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onLoupePointerDown(e) {
    // Above 100% the photo already overflows its frame — that drag now pans
    // it instead (see onPhotoPointerDown in +page.svelte), so the loupe only
    // claims the gesture while the photo still fits inside its margin.
    if (zoomMode !== "frame" || developPhotoPercent > 100 || isCropping || imgFailed || e.button !== 0) return;
    const source = loupeSource();
    // Only clicks landing ON the actual photo element start the loupe — not
    // the empty margin around it inside this flex-centered <main>.
    if (!source || e.target !== source.el) return;
    loupeActive = true;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    updateLoupe(e.clientX, e.clientY);
    e.stopPropagation();
  }

  /** @param {PointerEvent} e */
  function onLoupePointerMove(e) {
    if (!loupeActive) return;
    updateLoupe(e.clientX, e.clientY);
  }

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onLoupePointerUp(e) {
    if (!loupeActive) return;
    loupeActive = false;
    e.currentTarget.releasePointerCapture?.(e.pointerId);
  }

  /** @param {number} clientX @param {number} clientY */
  function updateLoupe(clientX, clientY) {
    const source = loupeSource();
    if (!source) return;
    const rect = source.el.getBoundingClientRect();
    if (!rect.width || !rect.height) return;

    // object-fit: contain picks the smaller scale (letterboxed), cover picks
    // the larger (overflowing) — same box, opposite constrained axis.
    const imgRatio = source.w / source.h;
    const boxRatio = rect.width / rect.height;
    const widthConstrained = objectFit === "cover" ? boxRatio > imgRatio : boxRatio <= imgRatio;
    const dispW = widthConstrained ? rect.width : rect.height * imgRatio;
    const dispH = widthConstrained ? rect.width / imgRatio : rect.height;
    const offX = (rect.width - dispW) / 2;
    const offY = (rect.height - dispH) / 2;

    let nx = (clientX - rect.left - offX) / dispW;
    let ny = (clientY - rect.top - offY) / dispH;
    if (nx < 0 || nx > 1 || ny < 0 || ny > 1) return; // dragged off the photo
    if (flipH < 0) nx = 1 - nx;
    if (flipV < 0) ny = 1 - ny;

    loupePos = { x: clientX, y: clientY };

    if (!loupeCanvasEl) return;
    const ctx = loupeCanvasEl.getContext("2d");
    if (!ctx) return;
    loupeCanvasEl.width = LOUPE_DIAMETER;
    loupeCanvasEl.height = LOUPE_DIAMETER;
    const span = LOUPE_DIAMETER / LOUPE_ZOOM;
    const sx = nx * source.w - span / 2;
    const sy = ny * source.h - span / 2;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, LOUPE_DIAMETER, LOUPE_DIAMETER);
    ctx.drawImage(source.el, sx, sy, span, span, 0, 0, LOUPE_DIAMETER, LOUPE_DIAMETER);
  }
</script>

<main
  class="zoom-{zoomMode}"
  class:panning
  class:frame-overflow={zoomMode === "frame" && developPhotoPercent > 100}
  class:has-caption={showCaption && caption.trim() && !imgFailed}
  class:loupe-open={loupeActive}
  onpointerdown={(e) => { onPhotoPointerDown(e); onLoupePointerDown(e); }}
  onpointermove={(e) => { onPhotoPointerMove(e); onLoupePointerMove(e); }}
  onpointerup={(e) => { onPhotoPointerUp(e); onLoupePointerUp(e); }}
  onpointercancel={(e) => { onPhotoPointerUp(e); onLoupePointerUp(e); }}
  oncontextmenu={(e) => e.preventDefault()}
>
  {#if busyVisible}
    <div class="render-badge" class:with-label={!!status}>
      <span class="render-spinner"></span>
      {#if status}<span class="din render-label">{status}</span>{/if}
    </div>
  {/if}

  {#if imgFailed}
    <div class="photo-mat photo-fallback">
      <Icon name="image-broken" size="44px" />
      <span class="fallback-name">{picked}</span>
      <span class="fallback-hint">Preview unavailable</span>
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
      <span class="legend-item red">● Blown highlights</span>
      <span class="legend-item blue">● Blocked shadows</span>
    </div>
  {/if}

  {#if showCaption && caption.trim() && !imgFailed}
    <div class="caption-plate">
      <div class="caption-bar">{@html captionHtml}</div>
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

{#if loupeActive}
  <div class="loupe" style="left: {loupePos.x}px; top: {loupePos.y}px;">
    <canvas bind:this={loupeCanvasEl} width={LOUPE_DIAMETER} height={LOUPE_DIAMETER}></canvas>
    <div class="loupe-crosshair"></div>
  </div>
{/if}

<style>
  .loupe {
    position: fixed;
    width: 220px;
    height: 220px;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    overflow: hidden;
    pointer-events: none;
    z-index: 50;
    border: 2px solid rgba(255, 255, 255, 0.85);
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.5), 0 8px 32px rgba(0, 0, 0, 0.5);
    background: #000;
  }
  .loupe canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
  /* The OS cursor draws above every element, so it sits over the glass and
     hides the very pixel being inspected. The crosshair is the cursor while
     the loupe is up. */
  main.loupe-open,
  main.loupe-open :global(*) {
    cursor: none;
  }
  /* The two arms are centred on the whole circle, not nested inside each
     other: an 11px bar centred with `margin:auto` inside a 1px-wide parent
     has negative free space, and CSS resolves that by zeroing margin-left
     and pushing the whole overflow right — which cost the crosshair its
     left arm (Francis, 2026-09-21). */
  .loupe-crosshair {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .loupe-crosshair::before,
  .loupe-crosshair::after {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    background: rgba(255, 255, 255, 0.7);
    box-shadow: 0 0 1px rgba(0, 0, 0, 0.6);
  }
  .loupe-crosshair::before {
    width: 1px;
    height: 11px;
    transform: translate(-50%, -50%);
  }
  .loupe-crosshair::after {
    width: 11px;
    height: 1px;
    transform: translate(-50%, -50%);
  }

  main {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    overflow: hidden;
    background: var(--color-background, #121212);
  }
  /* With a caption, the photo and its plate stack as two ordinary flex
     items in a column — the photo shrinks to leave the caption room below
     it rather than the caption sitting on top of it (matStyle's own
     max-height:90% still caps the photo, so it just settles a bit smaller). */
  main.has-caption {
    flex-direction: column;
    gap: 14px;
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
    /* A plain white line disappears over a bright sky/highlight in the
       photo underneath it — drop-shadow (unlike box-shadow, which follows
       the div's box, mostly transparent here) hugs the actual rendered
       border pixels, giving the L-bracket a dark outline that keeps it
       visible over light AND dark image content. */
    filter: drop-shadow(0 0 1px rgba(0, 0, 0, 0.9)) drop-shadow(0 0 1px rgba(0, 0, 0, 0.9));
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

  /* A flow item below the photo, not an overlay on it — see main.has-caption. */
  .caption-plate {
    flex-shrink: 0;
    max-width: 90%;
    display: flex;
    justify-content: center;
  }
  .caption-bar {
    width: auto;
    max-width: 46rem;
    box-sizing: border-box;
    padding: 0 4px;
    color: var(--color-foreground, #fff);
    font-family: var(--font-text, serif);
    font-size: 15px;
    line-height: 1.45;
    text-align: center;
    white-space: pre-wrap;
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

  /* Photo Size slider (DevTab) pushed past 100% — same drag-to-pan
     mechanism as zoom-actual (scrollLeft/scrollTop, see onPhotoPointerDown
     in +page.svelte), just gated to when there's actually somewhere to pan. */
  main.frame-overflow {
    overflow: auto;
    cursor: grab;
  }
  main.frame-overflow.panning {
    cursor: grabbing;
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
    top: 14px;
    right: 14px;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 7px;
    color: rgba(255, 255, 255, 0.55);
    font-size: 0.68rem;
    animation: badge-in var(--duration-fast, 160ms) ease-out;
  }
  /* Only a message worth reading earns the backing pill; a plain "still
     working" spinner floats bare over the photo. */
  .render-badge.with-label {
    padding: 4px 10px;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(8px);
    border-radius: 999px;
  }
  @keyframes badge-in {
    from {
      opacity: 0;
    }
  }
  .render-spinner {
    width: 9px;
    height: 9px;
    border: 1.5px solid rgba(255, 255, 255, 0.18);
    border-top-color: rgba(255, 255, 255, 0.7);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
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
