<script>
  // The grid cell — a 1:1 port of the Swift `CullCell` (CullView.swift).
  // Every photo wears its quiet print-frame (photoFrame ring, 3px). Selected
  // and collected read as ONE state: light mode deepens the drop-shadow, dark
  // mode re-colours the ring to a visible gray — never an accent border, the
  // chrome must not pull the eye off the image.
  import Icon from "$lib/components/Icon.svelte";

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
    fill = true,
    idx
  } = /** @type {{ path: string, name: string, previewVersion?: number, rating?: number, selected?: boolean, inStory?: boolean, isExporting?: boolean, isRendering?: boolean, onSelect: (e: MouseEvent) => void, onDblClick: (e: MouseEvent) => void, onContextMenu: (e: MouseEvent) => void, onToggleStory: () => void, onDragStart: (e: DragEvent) => void, layout?: string, aspect?: number, fill?: boolean, idx?: number }} */ ($props());

  let loaded = $state(false);
  let failed = $state(false);
  let naturalAspect = $state(1.5);
  let imgEl = $state();

  function handleLoad() {
    loaded = true;
    if (imgEl && imgEl.naturalWidth && imgEl.naturalHeight) {
      naturalAspect = imgEl.naturalWidth / imgEl.naturalHeight;
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

  /**
   * @param {string} p
   * @param {number} version
   */
  function thumbUrl(p, version) {
    return `reveal://thumb?p=${encodeURIComponent(p)}&v=${version}`;
  }

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
  draggable="true"
  ondragstart={onDragStart}
  onclick={onSelect}
  ondblclick={onDblClick}
  oncontextmenu={onContextMenu}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect();
    }
  }}
  role="button"
  tabindex="0"
  style="aspect-ratio: {layout === 'masonry' ? naturalAspect : aspect};"
>
  <div class="matte" class:loaded>
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
  </div>

  <!-- Stars: fill = the app background, stroke = the hairline — the one star
       look everywhere (Swift `Stars`), no black chip behind. -->
  {#if rating > 0}
    <div class="stars-overlay">
      {#each Array(rating) as _}
        <svg class="star" viewBox="0 0 24 24" width="11" height="11">
          <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z" />
        </svg>
      {/each}
    </div>
  {/if}

  <!-- Story dot: in-story = a quiet bg-coloured marker (not a loud red dot);
       hover-only when it isn't, so empty dots don't clutter the grid. -->
  <button
    class="story-dot-btn"
    class:in-story={inStory}
    onclick={(e) => {
      e.stopPropagation();
      onToggleStory();
    }}
    title={inStory ? "Retirer de l'histoire" : "Ajouter à l'histoire"}
    aria-label={inStory ? "Retirer de l'histoire" : "Ajouter à l'histoire"}
  ></button>

  {#if isExporting || isRendering}
    <div class="render-badge" title={isExporting ? "Export en cours…" : "Rendu en cours…"}>
      <span class="badge-dot"></span>
    </div>
  {/if}

  <!-- Hover caption — always the DARK palette's scrim + light text, even in
       light mode (a white wash is unreadable over a bright photo). -->
  <div class="caption-overlay">
    <span class="name">{stem(name)}</span>
  </div>
</div>

<style>
  .cell {
    all: unset;
    cursor: pointer;
    position: relative;
    display: block;
    width: 100%;
    box-sizing: border-box;
    border: 3px solid var(--color-surface);
    border-radius: var(--radius);
    background: var(--color-surface);
    /* The quiet print-on-a-table depth. */
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.28);
    transition: box-shadow var(--duration-standard) var(--ease-soft), border-color var(--duration-standard) var(--ease-soft);
    overflow: hidden;
    /* Own compositing layer: on the translucent window WKWebView otherwise
       smears each cell's box-shadow sideways into the next grid column
       (a shared-layer paint bug). Isolating the cell contains its shadow. */
    transform: translateZ(0);
    isolation: isolate;
  }

  /* Selected, light mode: the print lifts — a harder, darker drop. */
  .cell.selected {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.75);
  }
  /* Selected, dark mode: the print-frame just changes colour (same width),
     and the shadow stays at the quiet depth. */
  @media (prefers-color-scheme: dark) {
    .cell.selected {
      border-color: rgb(82, 82, 82);
      box-shadow: 0 1px 3px rgba(0, 0, 0, 0.28);
    }
  }

  /* Mid-export — a soft accent aura breathing BEHIND the print, not a mark on it. */
  .cell.exporting {
    animation: export-breathe 2.2s infinite var(--ease-standard);
  }
  @keyframes export-breathe {
    0%, 100% { box-shadow: 0 1px 3px rgba(0, 0, 0, 0.28), 0 0 8px rgba(214, 32, 44, 0.25); }
    50% { box-shadow: 0 1px 3px rgba(0, 0, 0, 0.28), 0 0 16px rgba(214, 32, 44, 0.7); }
  }

  .cell:active {
    opacity: 1 !important;
  }

  .matte {
    width: 100%;
    height: 100%;
    position: relative;
    overflow: hidden;
    border-radius: var(--radius-sm);
  }

  .matte img {
    width: 100%;
    height: 100%;
    display: block;
    opacity: 0;
    transition: opacity var(--duration-instant) var(--ease-soft);
    pointer-events: none;
    -webkit-user-drag: none;
    -webkit-user-select: none;
    user-select: none;
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
    background: color-mix(in srgb, var(--color-foreground) 6%, transparent);
  }
  .placeholder.failed {
    opacity: 0.32;
  }

  .stars-overlay {
    position: absolute;
    top: 8px;
    left: 8px;
    display: flex;
    gap: 2px;
    z-index: 10;
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.45));
    pointer-events: none;
  }
  .star {
    fill: var(--color-background);
    stroke: var(--color-border);
    stroke-width: 1.2;
    stroke-linejoin: round;
  }

  .story-dot-btn {
    all: unset;
    position: absolute;
    top: 8px;
    right: 8px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.28);
    border: 1.5px solid var(--color-border);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.45);
    z-index: 10;
    opacity: 0;
    transition: opacity var(--duration-standard) var(--ease-soft), background-color var(--duration-instant) var(--ease-soft);
    cursor: pointer;
  }
  .cell:hover .story-dot-btn,
  .story-dot-btn.in-story {
    opacity: 1;
  }
  .story-dot-btn.in-story {
    background: var(--color-background);
  }

  .render-badge {
    position: absolute;
    bottom: 8px;
    right: 8px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--color-surface-high);
    border: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
    z-index: 10;
  }
  .badge-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-accent);
  }

  /* Just tall enough to seat the caption — not a wash over half the photo —
     toned to the dark palette's photoFrame (#171717), not flat black. */
  .caption-overlay {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 10;
    background: linear-gradient(to top, rgba(23, 23, 23, 0.92) 20%, rgba(23, 23, 23, 0));
    padding: 18px 8px 6px;
    display: flex;
    flex-direction: column;
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--duration-instant) var(--ease-soft);
  }
  .cell:hover .caption-overlay {
    opacity: 1;
  }
  .caption-overlay .name {
    font-family: var(--font-text, sans-serif);
    font-size: 10px;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
