<script>
  // The contact-sheet grid — the Swift `uniformGrid` geometry (photos cap at
  // 360px, slack flows into equal gutters, explicit column count), now
  // VIRTUALIZED: cell size is pure geometry (width/cols, height by aspect),
  // so only the rows in the viewport (± a small buffer) exist in the DOM.
  // "Toute la bibliothèque" is ~22k frames — rendering them all is why the
  // root froze. Masonry keeps natural ratios (no fixed geometry) and is NOT
  // virtualized; it's the opt-in layout for day-sized folders. Keep the
  // container class app-specific: Standard's global `.grid` utility flows
  // children by column, which would turn this contact sheet into one long row.
  import PhotoCell from "./PhotoCell.svelte";
  import { untrack } from "svelte";

  /** @typedef {{ path: string, name: string, previewVersion?: number, rating?: number }} PhotoFrame */

  let {
    frames = [],
    sel = 0,
    selectedPaths = new Set(),
    storySet = new Set(),
    layout = "uniform",
    cols = 4,
    marginScale = 1,
    aspect = 1.5,
    fill = true,
    progress = null,
    onSelect = () => {},
    onDblClick = () => {},
    onContextMenu = () => {},
    onToggleStory = () => {},
    onDragStart = () => {},
    scrollTop = 0,
    onScroll = () => {}
  } = $props();

  /** @type {HTMLDivElement | null} */
  let gridElement = $state(null);
  let viewW = $state(1200);
  let viewH = $state(700);
  // svelte-ignore state_referenced_locally
  let top = $state(scrollTop);

  const safeViewW = $derived(viewW > 0 ? viewW : 1200);
  const safeViewH = $derived(viewH > 0 ? viewH : 700);

  const BUFFER_ROWS = 3;
  const gap = $derived(Math.round(16 * marginScale));
  const cellW = $derived.by(() => {
    const usable = safeViewW - gap * 2 - gap * (cols - 1);
    return Math.min(360, Math.max(48, usable / cols));
  });
  const rowH = $derived(cellW / aspect);
  const rowPitch = $derived(rowH + gap);
  const totalRows = $derived(Math.ceil(frames.length / cols));
  const virtual = $derived(layout !== "masonry");

  const firstRow = $derived(
    virtual ? Math.max(0, Math.floor(top / rowPitch) - BUFFER_ROWS) : 0,
  );
  const lastRow = $derived(
    virtual
      ? Math.min(totalRows - 1, Math.ceil((top + safeViewH) / rowPitch) + BUFFER_ROWS)
      : totalRows - 1,
  );
  const start = $derived(virtual ? firstRow * cols : 0);
  /** @type {PhotoFrame[]} */
  const slice = $derived(
    virtual ? frames.slice(start, (lastRow + 1) * cols) : frames,
  );
  // The rows scrolled past / still below live as padding, so the scrollbar
  // and positions are those of the full set.
  const padTop = $derived(virtual ? firstRow * rowPitch : 0);
  const padBottom = $derived(
    virtual ? Math.max(0, (totalRows - 1 - lastRow) * rowPitch) : 0,
  );

  let isProgrammaticScroll = false;

  // Parent-driven scroll restore (per-folder memory).
  $effect(() => {
    const limit = virtual ? Math.max(0, totalRows * rowPitch + 64 - safeViewH) : Infinity;
    const target = Math.max(0, Math.min(scrollTop, limit));
    if (gridElement && (Math.abs(gridElement.scrollTop - target) > 2 || top !== target)) {
      isProgrammaticScroll = true;
      top = target;
      gridElement.scrollTop = target;
      if (target !== scrollTop) untrack(() => onScroll(target));
      setTimeout(() => {
        isProgrammaticScroll = false;
      }, 50);
    }
  });

  // Keyboard nav: the selected cell may not exist in the DOM yet, so keep it
  // visible by geometry, never by scrollIntoView.
  /** @type {number | undefined} */
  let lastFocusedIndex;
  $effect(() => {
    const focusedIndex = sel;
    const element = gridElement;
    if (!element || lastFocusedIndex === focusedIndex) return;
    const initial = lastFocusedIndex === undefined;
    lastFocusedIndex = focusedIndex;
    untrack(() => {
      // A remount restores the stored viewport, not the last selection. Only
      // selection changes should pull a manually scrolled grid back to a cell.
      if ((initial && scrollTop > 0) || !virtual || !frames.length || focusedIndex < 0) return;
      const row = Math.floor(focusedIndex / cols);
      const y0 = row * rowPitch;
      const y1 = y0 + rowH + gap;
      const viewportHeight = safeViewH;
      const st = element.scrollTop;
      const target = y0 < st ? y0 : y1 > st + viewportHeight ? y1 - viewportHeight : null;
      if (target === null) return;
      isProgrammaticScroll = true;
      top = target;
      element.scrollTop = target;
      onScroll(target);
      setTimeout(() => { isProgrammaticScroll = false; }, 50);
    });
  });

  function handleScroll() {
    if (gridElement) {
      if (isProgrammaticScroll) return;
      const newTop = gridElement.scrollTop;
      if (Math.abs(newTop - top) > 1) {
        top = newTop;
        onScroll(newTop);
      }
    }
  }

  /**
   * @param {PhotoFrame} f
   */
  function isExporting(f) {
    if (!progress || progress.verb !== "export") return false;
    return progress.current === f.name || stem(progress.current) === stem(f.name);
  }

  /** @param {string} n */
  function stem(n) {
    if (!n) return "";
    return n.replace(/\.[^.]+$/, "");
  }

  $effect(() => {
    console.debug(`PhotoGrid geom: viewW=${viewW} (safe=${safeViewW}) viewH=${viewH} (safe=${safeViewH}) cols=${cols} sliceLen=${slice.length} totalRows=${totalRows} framesLen=${frames.length} virtual=${virtual} layout=${layout}`);
  });
</script>

<div
  class="grid-wrapper"
  bind:clientWidth={viewW}
  bind:clientHeight={viewH}
  bind:this={gridElement}
  onscroll={handleScroll}
>
  <div
    class="photo-grid"
    class:masonry={layout === "masonry"}
    style="--cols: {cols}; --gap: {gap}px; --cellw: {cellW}px; padding-top: {gap / 2 +
      padTop}px; padding-bottom: {64 + padBottom}px"
  >
    {#each slice as f, j (f.path)}
      <PhotoCell
        path={f.path}
        name={f.name}
        previewVersion={f.previewVersion}
        rating={f.rating}
        selected={selectedPaths.has(f.path)}
        inStory={storySet.has(stem(f.name))}
        isExporting={isExporting(f)}
        {layout}
        {aspect}
        {fill}
        idx={start + j}
        onSelect={(/** @type {MouseEvent | undefined} */ event) => onSelect(start + j, event)}
        onDblClick={() => onDblClick(f.path)}
        onContextMenu={(/** @type {MouseEvent} */ event) => onContextMenu(start + j, event)}
        onToggleStory={() => onToggleStory(f.path)}
        onDragStart={(/** @type {DragEvent} */ event) => onDragStart(f.path, event)}
      />
    {/each}
  </div>
</div>

<style>
  .grid-wrapper {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    position: relative;
    overflow-y: auto;
  }

  .photo-grid {
    display: grid;
    grid-template-columns: repeat(var(--cols), var(--cellw));
    grid-auto-flow: row;
    grid-auto-columns: unset;
    justify-content: center;
    gap: var(--gap);
    padding-left: var(--gap);
    padding-right: var(--gap);
  }

  /* Masonry — native CSS columns at the same explicit column count. */
  .photo-grid.masonry {
    display: block;
    column-count: var(--cols);
    column-gap: var(--gap);
  }
  .photo-grid.masonry :global(.cell) {
    margin-bottom: var(--gap);
    break-inside: avoid;
    width: 100%;
    /* A transform (the shadow-isolation trick used in the uniform grid) turns
       a cell into a containing block, which breaks CSS multi-column flow —
       cells land in the wrong column or vanish. Masonry uses columns, so drop
       it here; the sideways-shadow smear it fixes only shows in the uniform
       grid's centered tracks anyway. */
    transform: none;
    isolation: auto;
  }
</style>
