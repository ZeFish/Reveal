<script>
  import { Icon } from "@modules/core";
  import { createScopeAnalyzer } from "./developAnalysis.js";
  import { nextStableWidth, displayBox } from "./stableDisplayWidth.js";
  import CaptionOverlay from "./CaptionOverlay.svelte";
  import CheckLayerOverlay from "./CheckLayerOverlay.svelte";
  import CropOverlay from "./CropOverlay.svelte";
  import DevelopLoupe from "./DevelopLoupe.svelte";

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
    checkLayer = "none",
    /** @type {(mode: string) => void} */
    onSelectCheckLayer = () => {},
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
    /** @type {(live: boolean) => void} */
    onCropChange = () => {},
    onPhotoPointerDown = () => {},
    onPhotoPointerMove = () => {},
    onPhotoPointerUp = () => {},
    /** The photo's own volume is unreachable; what is on screen came from the
     * local cache. A persistent condition, so it gets a persistent mark in
     * the one corner already reserved for "what is this view doing", rather
     * than a toast that fades while the condition does not. */
    sourceOffline = false,
    /** @type {'shadows' | 'midtones' | 'highlights' | null} */
    zoneMask = $bindable(null),
    /** Before / after: the photo as the camera shot it, laid over the developed one while shown. */
    showBefore = false,
    /** @type {string | null} */
    beforeUrl = null,
  } = $props();

  let frameCap = $derived(
    zoomMode === "frame"
      ? `max-width: ${developPhotoPercent}%; max-height: ${developPhotoPercent}%;`
      : "",
  );

  // Now that the caches usually answer instantly, a render that finishes in
  // 30ms used to flash a full pill on screen — more distracting than the wait
  // it announced. So: nothing at all for the first BUSY_DELAY_MS, then a bare
  // wordless spinner. Words belong to the notification stack at the bottom of
  // the window, which is the one place the user has to look.
  const BUSY_DELAY_MS = 400;
  let loaded = $state(false);
  let busy = $derived(
    inflight || pendingPx !== null || (!loaded && !!imgUrl && !imgFailed && !useCanvas)
  );
  let busyVisible = $state(false);
  /** Is there any image in the frame right now? Drives where the busy mark
   * goes — a corner is polite over a photo and invisible over nothing. */
  let hasSomethingOnScreen = $derived(useCanvas || (loaded && !imgFailed));
  $effect(() => {
    if (!busy) {
      busyVisible = false;
      return;
    }
    const t = setTimeout(() => (busyVisible = true), BUSY_DELAY_MS);
    return () => clearTimeout(t);
  });

  /** @type {HTMLImageElement | null} */ let imgEl = $state(null);
  let effectiveCheckLayer = $derived(
    zoneMask ? `zone_${zoneMask}` : (checkLayer !== "none" ? checkLayer : (showClipping ? "clipping" : "none"))
  );
  let aspectRatio = $state("");
  // See stableDisplayWidth.js: the width the photo is drawn at, whatever resolution
  // the render in front of us happens to be.
  /** @type {{ key: string, w: number }} */
  let stable = $state({ key: "", w: 0 });
  let frameW = $state(0);
  let frameH = $state(0);
  // Only where the plain caps do the right thing today: the "frame" zoom, no caption
  // stacked under the photo, no crop or aspect override, not mid-crop.
  const stableStyle = $derived.by(() => {
    if (zoomMode !== "frame" || isCropping || objectFit !== "contain") return "";
    if (showCaption && caption.trim()) return "";
    const box = displayBox({ frameW, frameH, percent: developPhotoPercent, stable });
    return box ? `width: ${box.w}px; height: ${box.h}px;` : "";
  });
  $effect(() => {
    picked; // another photo: start over, a thumbnail must not be drawn at the render's size
    stable = { key: "", w: 0 };
  });

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
  let isCropping = $derived(showCropOverlay);
  let rotate = $derived(isCropping ? (recipe?.crop_angle || 0) : 0);

  // Right to left, as the engine does it: the picture is turned (straighten), then mirrored.
  let transformStr = $derived(`scaleX(${flipH}) scaleY(${flipV}) rotate(${rotate}deg)`);
  // The crop frame is mirrored with the picture but never turned: it is upright in the
  // straightened picture, which is what the engine cuts out.
  let frameStr = $derived(`scaleX(${flipH}) scaleY(${flipV})`);

  // The Crop tab's zoom/pan (CropOverlay computes it): the crop frame stays centred and the
  // picture moves behind it, so the photo wears the same view the overlay does.
  let cropViewTransform = $state("");
  let cropViewTransition = $state("");

  let matStyle = $derived(
    `${frameCap} ${cropAspect ? `aspect-ratio: ${cropAspect};` : ""} object-fit: ${objectFit}; transform: ${
      isCropping && cropViewTransform ? `${cropViewTransform} ` : ""
    }${transformStr};${isCropping && cropViewTransition ? ` transition: ${cropViewTransition};` : ""}`
  );

  const photoEl = $derived(useCanvas ? canvasEl : imgEl);

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

  const scopeAnalyzer = createScopeAnalyzer();

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

  function updateHistogram() {
    const source = getSourcePixelData();
    if (!source) return;
    const result = scopeAnalyzer.analyze({
      width: source.width,
      height: source.height,
      data: source.data.data,
    });
    histogram = result.histogram;
    scopes = result.scopes;
  }

  /** @param {Event} e */
  function handleLoad(e) {
    imgFailed = false;
    loaded = true;
    const target = /** @type {HTMLImageElement} */ (e.currentTarget) || imgEl;
    if (target?.naturalWidth && target?.naturalHeight) {
      aspectRatio = `${target.naturalWidth} / ${target.naturalHeight}`;
      // A blob: URL is a develop render; reveal://thumb is the grid's thumbnail.
      const rendered = (target.currentSrc || target.src || "").startsWith("blob:");
      stable = nextStableWidth(stable, target.naturalWidth, target.naturalHeight, rendered);
    }
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

  // Capture One-style loupe: press on the photo, a circle follows the
  // cursor showing that spot at native pixel scale (no upscaling blur —
  // imageSmoothingEnabled off), release to dismiss. Only in "frame" zoom
  // (fit-to-view) — "actual" already shows 1:1 pixels and owns pointer-down
  // for panning (see the prop handlers above), and crop owns its own
  // overlay's pointer events entirely.
  let loupeActive = $state(false);
  let loupePos = $state({ x: 0, y: 0 }); // client coords, for the fixed overlay
  let loupeNorm = $state({ x: 0.5, y: 0.5 });
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
    e.preventDefault();
    e.stopPropagation();
    return true;
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
    loupeNorm = { x: nx, y: ny };
  }
</script>

<main
  bind:clientWidth={frameW}
  bind:clientHeight={frameH}
  class="zoom-{zoomMode}"
  class:panning
  class:frame-overflow={zoomMode === "frame" && developPhotoPercent > 100}
  class:has-caption={showCaption && caption.trim() && !imgFailed}
  class:loupe-open={loupeActive}
  onpointerdown={(e) => { if (!onLoupePointerDown(e)) onPhotoPointerDown(e); }}
  onpointermove={(e) => { onPhotoPointerMove(e); onLoupePointerMove(e); }}
  onpointerup={(e) => { onPhotoPointerUp(e); onLoupePointerUp(e); }}
  onpointercancel={(e) => { onPhotoPointerUp(e); onLoupePointerUp(e); }}
  oncontextmenu={(e) => e.preventDefault()}
>
  {#if sourceOffline}
    <div
      class="render-badge text-accent"
      role="status"
      title="This folder is unreachable — showing the cached copy"
    >
      <Icon name="link-break" size="var(--icon-md)" />
    </div>
  {:else if busyVisible}
    <div class="render-badge" class:centred={!hasSomethingOnScreen}>
      <span class="loader"></span>
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
      style="{matStyle} {stableStyle}"
      data-no-zoom
      draggable="false"
      src={imgUrl}
      alt={picked}
      onerror={() => (imgFailed = true)}
      onload={handleLoad}
    />
  {:else}
    <div class="empty-stage">
      <Icon name="aperture" size="40px" />
      <span class="empty-name">Reveal</span>
      {#if status}<span class="empty-status">{status}</span>{/if}
    </div>
  {/if}

  {#if showBefore && beforeUrl && !imgFailed && !isCropping}
    <img
      class="photo-mat before-layer"
      style="{matStyle} {stableStyle}"
      src={beforeUrl}
      alt=""
      aria-hidden="true"
      draggable="false"
    />
    <span class="before-chip" role="status">Before</span>
  {/if}

  {#if effectiveCheckLayer !== "none" && !imgFailed}
    <CheckLayerOverlay
      {matStyle}
      {effectiveCheckLayer}
      {zoneMask}
      {getSourcePixelData}
      {canvasVersion}
      {onSelectCheckLayer}
      onCloseZoneMask={() => { zoneMask = null; }}
    />
  {/if}

  {#if showCaption && caption.trim() && !imgFailed}
    <CaptionOverlay {caption} />
  {/if}

  {#if isCropping && !imgFailed}
    <CropOverlay
      {photoEl}
      {recipe}
      {renderAspect}
      transformStr={frameStr}
      {onCropChange}
      bind:viewTransform={cropViewTransform}
      bind:viewTransition={cropViewTransition}
    />
  {/if}
</main>

<DevelopLoupe
  active={loupeActive}
  pos={loupePos}
  norm={loupeNorm}
  source={loupeSource()}
  diameter={LOUPE_DIAMETER}
  zoom={LOUPE_ZOOM}
/>

<style>
  main {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    overflow: hidden;
  }

  /* With a caption, the photo and its plate stack as two ordinary flex
     items in a column — the photo shrinks to leave the caption room below
     it rather than the caption sitting on top of it. */
  main.has-caption {
    flex-direction: column;
    gap: var(--space);
  }

  .photo-mat {
    display: block;
    box-sizing: border-box;
    max-width: 90%;
    max-height: 90%;
    width: auto;
    height: auto;
    border: none;
    /* The photo's own corner is `--radius`, whatever the engine draws it with: a border-radius
       on a box with padding only leaves `radius - padding` for the picture inside, which was
       about nothing for an image and something else for a canvas. So the mat's corner is the
       photo's plus the padding it wraps it in. */
    --mat-pad: calc(var(--space-d4) * 3);
    border-radius: calc(var(--radius) + var(--mat-pad));
    object-fit: contain;
    padding: var(--mat-pad);
    background: var(--color-surface-raised);
    box-shadow: var(--shadow), var(--shadow-glow);
    transition: all var(--transition-fast);
    -webkit-user-drag: none;
    -webkit-user-select: none;
    user-select: none;
  }
  .photo-mat:active {
    position: static;
  }

  /* The photo as shot, exactly over the developed one: same box, nothing of its own. */
  .photo-mat.before-layer {
    position: absolute;
    z-index: 6;
    pointer-events: none;
    background: transparent;
  }
  .before-chip {
    position: absolute;
    top: var(--space-d2);
    left: var(--space-d2);
    z-index: 7;
    padding: 2px var(--space-d3);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--color-background) 80%, transparent);
    color: var(--color-foreground);
    font-family: var(--font-monospace);
    font-size: 0.62rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    pointer-events: none;
  }

  .photo-mat img { display: none; }

  .photo-mat.dimmed {
    opacity: 0.75;
  }

  /* OS cursor hidden while loupe crosshair is active */
  main.loupe-open,
  main.loupe-open :global(*) {
    cursor: none;
  }

  .photo-fallback {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-d2);
    min-width: 14rem;
    min-height: 10rem;
    color: var(--color-muted);
  }
  .fallback-hint {
    opacity: 0.7;
  }

  main.zoom-fill .photo-mat {
    max-width: 100%;
    max-height: 100%;
  }

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
    gap: var(--space-d2);
    animation: badge-in var(--duration-fast, 160ms) ease-out;
  }
  .render-badge.centred {
    top: 50%;
    right: auto;
    left: 50%;
    transform: translate(-50%, -50%);
  }
  @keyframes badge-in {
    from {
      opacity: 0;
    }
  }

  .empty-stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--space-d4) * 3);
    color: var(--color-subtle);
    user-select: none;
  }
  .empty-status {
    opacity: 0.8;
  }
</style>
