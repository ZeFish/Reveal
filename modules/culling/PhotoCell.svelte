<script>
  // The grid cell — a 1:1 port of the Swift `CullCell` (CullView.swift).
  // Every photo wears its quiet print-frame (photoFrame ring, 3px). Selected
  // and collected read as ONE state: light mode deepens the drop-shadow, dark
  // mode re-colours the ring to a visible gray — never an accent border, the
  // chrome must not pull the eye off the image.
  import Icon from "$lib/components/Icon.svelte";
  import { thumbUrl } from "$lib/thumbUrl.js";

  /**
   * @typedef {Object} Props
   * @property {string} path
   * @property {string} name
   * @property {number} [previewVersion]
   * @property {number} [rating]
   * @property {boolean} [selected]
   * @property {boolean} [inStory]
   * @property {boolean} [isExporting]
   * @property {boolean} [isRendering]
   * @property {(e?: MouseEvent) => void} [onSelect]
   * @property {(e: MouseEvent) => void} [onDblClick]
   * @property {(e: MouseEvent) => void} [onContextMenu]
   * @property {() => void} [onToggleStory]
   * @property {(e: DragEvent) => void} [onDragStart]
   * @property {string} [layout]
   * @property {number} [aspect]
   * @property {number} [knownAspect] the photo's own ratio, from the index
   * @property {boolean} [fill]
   * @property {number} [idx]
   */

  /** @type {Props} */
  let {
    path,
    name,
    previewVersion = 0,
    rating = 0,
    selected = false,
    inStory = false,
    isExporting = false,
    isRendering = false,
    onSelect = () => {},
    onDblClick = () => {},
    onContextMenu = () => {},
    onToggleStory = () => {},
    onDragStart = () => {},
    layout = "uniform",
    aspect = 1.5,
    knownAspect = undefined,
    fill = true,
    idx
  } = $props();

  let loaded = $state(false);
  let failed = $state(false);
  /**
   * What the loaded image turned out to be, once it has loaded. `null` until
   * then — and again whenever this cell is recycled for another photo, which
   * the virtual grid does constantly.
   * @type {number | null}
   */
  let measuredAspect = $state(null);
  /**
   * The photo's own ratio.
   *
   * Derived, not seeded: a `$state` initialiser captures its value once, and
   * a recycled cell would go on describing the photo it used to hold.
   *
   * The index answers first, so a folder can be laid out — portraits included
   * — before a single thumbnail has come back from the NAS. 1.5 is the guess
   * for a row indexed before that column existed, or a file libraw cannot
   * read; the image corrects whichever of the two it was.
   */
  const naturalAspect = $derived(measuredAspect ?? knownAspect ?? 1.5);
  /** Masonry gives the slot the photo's shape already, and `fill` crops to
   *  the slot on purpose; neither wants a card of its own. */
  const hugs = $derived(layout !== "masonry" && !fill);
  let imgEl = $state();

  function handleLoad() {
    loaded = true;
    if (imgEl && imgEl.naturalWidth && imgEl.naturalHeight) {
      measuredAspect = imgEl.naturalWidth / imgEl.naturalHeight;
    }
  }

  // Without this, a thumbnail that genuinely fails (unreadable RAW, moved
  // file) looks identical to one still decoding — same quiet placeholder,
  // forever. A distinct glyph tells the two apart.
  function handleError() {
    failed = true;
  }

  $effect(() => {
    if (imgEl && imgEl.complete) {
      handleLoad();
    }
  });


  /** @param {string} n */
  function stem(n) {
    return n.replace(/\.[^.]+$/, "");
  }
</script>

<div
  class="cell"
  class:selected
  class:exporting={isExporting}
  data-idx={idx}
  data-reveal-host
  draggable={!path.startsWith("apple-photos://")}
  ondragstart={onDragStart}
  onclick={onSelect}
  ondblclick={onDblClick}
  oncontextmenu={onContextMenu}
  onkeydown={(e) => {
    // Enter activates. Space is NOT handled here: it is the app's quick-look
    // key, and preventing its default on the cell hid it from the window-level
    // shortcut handler (which skips events already handled).
    if (e.key === "Enter") {
      e.preventDefault();
      onSelect();
    }
  }}
  role="button"
  tabindex="0"
  style="aspect-ratio: {layout === 'masonry' ? naturalAspect : aspect};"
>
  <!-- Which side binds is arithmetic, not a CSS guess: a photo wider than its
       slot is limited by width, a narrower one by height. Saying so outright
       beats `aspect-ratio` plus a definite dimension, which is a definite
       dimension with the other hung off it — the mistake that letterboxed
       landscapes and flattened masonry an hour ago. -->
  <div
    class="matte"
    class:loaded
    class:hug={hugs}
    style={hugs
      ? `aspect-ratio:${naturalAspect};${
          naturalAspect >= aspect ? "width:100%;height:auto" : "height:100%;width:auto"
        }`
      : ""}
  >
    <!-- The print inside the mat. It carries its own, CONCENTRIC corner
         (the mat's radius less the mat), so the photo's curve follows the
         card's instead of being cut by the card's outer arc. The overlays
         sit on the print, so they are placed from the photo's edge. -->
    <div class="print">
    <img
      data-no-zoom
      bind:this={imgEl}
      src={thumbUrl(path, previewVersion)}
      alt={name}
      loading="eager"
      decoding="async"
      draggable="false"
      onload={handleLoad}
      onerror={handleError}
      class:visible={loaded}
      style="object-fit: {layout === 'masonry' || fill ? 'cover' : 'contain'};"
    />
    {#if !loaded}
      <!-- Loading — a near-empty frame with one quiet centred glyph, not a
           heavy grey block (a screen of un-decoded cells reads as calm).
           A failed decode gets a distinct glyph so it doesn't read as
           "still loading" indefinitely. -->
      <div class="placeholder" class:failed aria-hidden="true">
        <Icon name={failed ? "image-broken" : "image"} size="26px" />
      </div>
    {/if}

    <!-- The overlays live INSIDE the print, not the slot: the card hugs the
         photo now, so anything anchored to the slot floated off its edges.
         The print's overflow + radius clips them to the photo. -->
    <!-- Stars: filled with the accent, edged a shade darker — no black chip
         behind. -->
    {#if rating > 0}
      <div class="stars-overlay">
        {#each Array(rating) as _}
          <svg class="star" viewBox="0 0 24 24" width="11" height="11">
            <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z" />
          </svg>
        {/each}
      </div>
    {/if}

    <!-- Story dot: in-story = the accent; hover-only when it isn't, so empty
         dots don't clutter the grid. -->
    <button
      class="story-dot-btn"
      data-reveal
      disabled={path.startsWith("apple-photos://")}
      hidden={path.startsWith("apple-photos://")}
      class:in-story={inStory}
      onclick={(e) => {
        e.stopPropagation();
        onToggleStory();
      }}
      title={inStory ? "Remove from story" : "Add to story"}
      aria-label={inStory ? "Remove from story" : "Add to story"}
    ></button>

    {#if isExporting || isRendering}
      <span class="loader render-badge" title={isExporting ? "Export en cours…" : "Rendu en cours…"}></span>
    {/if}

    </div>
    <!-- The file name sits UNDER the card, in the gap between rows, not over
         the photo (Francis, 2026-09-28): a label on the print hid the part
         of the frame you were looking at. Outside .print so its clip does
         not apply; the card itself no longer clips. -->
    <div class="caption" data-reveal>
      <span class="name">{stem(name)}</span>
    </div>
  </div>
</div>

<style>
  /* The SLOT, not the card. Its aspect stays the configured one because
     PhotoGrid derives rowPitch from that same value (`rowH = cellW / aspect`)
     — if this box took the photo's shape instead, the DOM rows and the
     virtual geometry would disagree and scrolling would drift. The card is
     `.matte`, which sizes itself to the photo inside this box. */
  .cell {
    all: unset;
    cursor: pointer;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    box-sizing: border-box;
    /* Own compositing layer: on the translucent window WKWebView otherwise
       smears each cell's box-shadow sideways into the next grid column
       (a shared-layer paint bug). Isolating the cell contains its shadow. */
    transform: translateZ(0);
    isolation: isolate;
  }

  /* Selected: the print lifts — the ring of --shadow-raised plus the
     framework's lift, both themes (the tokens already know the scheme).
     On the card, never on .cell: the cell is the invisible slot, and a
     shadow there outlined empty space once the card hugged the photo. */

  /* Mid-export — a soft accent aura breathing BEHIND the print, not a mark on it. */
  .cell.exporting {
    animation: export-breathe 2.2s infinite var(--ease-standard);
  }
  @keyframes export-breathe {
    0%, 100% { box-shadow: 0 1px 3px rgba(0, 0, 0, 0.28), 0 0 8px color-mix(in srgb, var(--color-accent) 25%, transparent); }
    50% { box-shadow: 0 1px 3px rgba(0, 0, 0, 0.28), 0 0 16px color-mix(in srgb, var(--color-accent) 70%, transparent); }
  }

  .cell:active {
    opacity: 1 !important;
  }

  .matte {
    width: 100%;
    height: 100%;
    position: relative;
    /* No overflow: hidden — .print clips the photo to its own concentric
       corner, and the caption hangs below the card. */
    box-sizing: border-box;
    /* The mat around the print: padding in the mat colour, not a border —
       the hairline edge comes from the shadow. */
    --mat: 6px;
    padding: var(--mat);
    background: var(--color-surface-light-1);
    border-radius: max(1px, var(--radius));
    /* The quiet print-on-a-table depth. */
    box-shadow: var(--shadow);
    transition:
    all var(--transition-fast);
    /* Own compositing layer: on the translucent window WKWebView otherwise
       smears each card's box-shadow sideways into the next grid column
       (a shared-layer paint bug). Isolating the card contains its shadow. */
    transform: translateZ(0);
    isolation: isolate;
  }
  .cell:hover .matte {
      background: var(--color-surface-light-2);
      box-shadow: var(--shadow-hover), var(--shadow-glow);
  }
  .cell.selected {
    z-index: 5;
  }
  .cell.selected .matte {
    box-shadow: var(--shadow-hover), var(--shadow-glow);
    transform: translateY(-1px) translateZ(0);
    background: var(--color-surface-light-3);
  }

  /* A border used to be the mat, and CSS rounds a border's inner edge for
     free (outer radius − border width). Padding gets no such curve: the
     card's outer arc cut the photo 4px in, so at a large radius the corners
     visibly failed to match (Francis, 2026-09-23). The print states the
     concentric radius itself — the same rule as --pane-radius. */
  .print {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    border-radius: max(0px, calc(var(--radius) - var(--mat)));
  }

  /* The card takes the photo's proportion instead of padding it with negative
     space either side (Francis, 2026-09-23, on a portrait frame). Its ratio
     and the side that binds are set inline — from the index when the scan
     knows the size, from the loaded image otherwise — so a folder lays out
     correctly, portraits included, before a thumbnail has arrived. */
  .matte.hug {
    max-width: 100%;
    max-height: 100%;
  }
  .matte img {
    width: 100%;
    height: 100%;
    display: block;
    opacity: 0;
    border:0;
    box-shadow: var(--shadow-inset);
    transition: all vavr(--transition-fast);
    pointer-events: none;
    -webkit-user-drag: none;
    -webkit-user-select: none;
    user-select: none;
    border-radius: 2px;
  }
  .matte img.visible,
  .matte.loaded img {
    opacity: 1 !important;
    transition: none !important;
  }

  .placeholder {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--color-foreground) 3.5%, transparent);
    color: var(--color-foreground);
    opacity: 0.16;
  }
  .matte:not(.loaded) {
    background: var(--color-surface-dark-1);
    box-shadow: var(--shadow-inset);
  }
  .placeholder.failed {
    opacity: 0.32;
  }

  .stars-overlay {
    position: absolute;
    top: 8px;
    left: 8px;
    display: flex;
    gap: var(--space-d8);
    z-index: 10;
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.45));
    pointer-events: none;
  }
  .star {
    fill: var(--color-accent);
    stroke: color-mix(in srgb, var(--color-accent) 55%, black);
    stroke-width: 1.2;
    stroke-linejoin: round;
  }

  .story-dot-btn {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 12px;
    height: 12px;
    z-index: 10;
    cursor: pointer;
  }
  .story-dot-btn.in-story {
    opacity: 1;
    background: var(--color-accent);
    border-color: color-mix(in srgb, var(--color-accent) 55%, black);
  }

  /* Just tall enough to seat the caption — not a wash over half the photo.
     Toned from the theme's deepest surface, with its foreground on top, so
     the scrim follows light and dark instead of always being black. */
  /* Below the card, centred on it, in the row gap. Shown on hover, as
     before; the lifted cell stays above the next row so the line is never
     covered. */
  .caption {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin-top: var(--space-d4);
    display: flex;
    justify-content: center;
    pointer-events: none;
  }
  .cell:hover {
    z-index: 6;
  }
  .caption .name {
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .render-badge {
    position: absolute;
    bottom: 8px;
    right: 8px;
    z-index: 10;
  }
</style>
