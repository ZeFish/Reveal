<script>
  // The contact-sheet grid — the Swift `uniformGrid` geometry (photos cap at
  // 360px, slack flows into equal gutters, explicit column count), now
  // VIRTUALIZED: cell size is pure geometry (width/cols, height by aspect),
  // so only the rows in the viewport (± a small buffer) exist in the DOM.
  // "All Library" is ~22k frames — rendering them all is why the
  // root froze. Masonry keeps natural ratios (no fixed geometry) and is NOT
  // virtualized; it's the opt-in layout for day-sized folders. Keep the
  // container class app-specific: Standard's global `.grid` utility flows
  // children by column, which would turn this contact sheet into one long row.
  import PhotoCell from "./PhotoCell.svelte";
  import { Icon, isTauri } from "@modules/core";
  import { invoke } from "@tauri-apps/api/core";
  import { untrack, onDestroy } from "svelte";
  import { visiblePaths } from "./visibleCells.js";

  /** @param {HTMLElement} node */
  function autofocus(node) {
    node.focus();
  }

  /** @typedef {{ path: string, name: string, previewVersion?: number, rating?: number, width?: number, height?: number }} PhotoFrame */

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
    onScroll = () => {},
    /** The top bar floats over the grid: the first row starts below it and the rows scroll under it. */
    underRail = false,
    /** @type {Map<number, {id: string, text: string}[]>} row (-1 = above the first) → paragraphs anchored there */
    gridProseByRow = new Map(),
    onSaveProse = () => {},
  } = $props();

  /** @type {HTMLDivElement | null} */
  let gridElement = $state(null);
  let viewW = $state(1200);
  let viewH = $state(700);
  // svelte-ignore state_referenced_locally
  let top = $state(scrollTop);

  const safeViewW = $derived(viewW > 0 ? viewW : 1200);
  const safeViewH = $derived(viewH > 0 ? viewH : 700);

  // How tall that bar is (a theme token, so read where it is defined): the grid keeps this much
  // room above its first row, and every position below is measured from the end of it.
  let inset = $state(0);
  $effect(() => {
    if (!underRail || typeof document === "undefined") {
      inset = 0;
      return;
    }
    const px = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--titlebar-height"));
    inset = Number.isFinite(px) ? px : 42;
  });
  /** Where the scroll is, counted from the end of the bar. */
  const scrolled = $derived(Math.max(0, top - inset));

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
    virtual ? Math.max(0, Math.floor(scrolled / rowPitch) - BUFFER_ROWS) : 0,
  );
  const lastRow = $derived(
    virtual
      ? Math.min(totalRows - 1, Math.ceil((scrolled + safeViewH) / rowPitch) + BUFFER_ROWS)
      : totalRows - 1,
  );
  const start = $derived(virtual ? firstRow * cols : 0);
  /** @type {PhotoFrame[]} */
  const slice = $derived(
    virtual ? frames.slice(start, (lastRow + 1) * cols) : frames,
  );
  // Tell the backend which cells are on screen: their thumbnails are read before the others'.
  // Debounced, so a fast scroll sends where it lands, not every row it crossed.
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let visibleTimer;
  $effect(() => {
    if (!isTauri || !virtual) return;
    const paths = visiblePaths(frames, { top, viewH: safeViewH, rowPitch, cols });
    clearTimeout(visibleTimer);
    visibleTimer = setTimeout(() => invoke("set_visible_thumbs", { paths }).catch(() => {}), 80);
  });
  onDestroy(() => {
    clearTimeout(visibleTimer);
    if (isTauri) invoke("set_visible_thumbs", { paths: [] }).catch(() => {});
  });
  // The rows scrolled past / still below live as padding, so the scrollbar
  // and positions are those of the full set.
  const padTop = $derived(virtual ? firstRow * rowPitch : 0);
  const padBottom = $derived(
    virtual ? Math.max(0, (totalRows - 1 - lastRow) * rowPitch) : 0,
  );

  // Masonry, placed by us rather than by CSS columns. `column-count` fills a
  // column top to bottom before starting the next, so the folder read down
  // the first column, then the second — and every image that measured itself
  // changed a height and let the browser re-balance the columns, cells
  // jumping between them. Here each photo goes, in order, to the currently
  // shortest column (reading order stays left to right), and the choice uses
  // only what the index already knows, so a photo loading never moves any
  // other. Indices into `frames`, one list per column.
  const MASONRY_FALLBACK_ASPECT = 1.5; // what PhotoCell assumes before it knows
  const masonryColumns = $derived.by(() => {
    if (layout !== "masonry") return /** @type {number[][]} */ ([]);
    const lists = /** @type {number[][]} */ (Array.from({ length: cols }, () => []));
    const heights = new Array(cols).fill(0);
    const gapUnits = gap / cellW;
    frames.forEach((f, i) => {
      let shortest = 0;
      for (let c = 1; c < cols; c++) if (heights[c] < heights[shortest] - 1e-6) shortest = c;
      lists[shortest].push(i);
      heights[shortest] += 1 / (f.width && f.height ? f.width / f.height : MASONRY_FALLBACK_ASPECT) + gapUnits;
    });
    return lists;
  });

  // Gaps between rows do two things: show whatever paragraphs already
  // anchor there (always — this is how you SEE a story while working the
  // grid, not just add to it), and — for gaps strictly between two visible
  // rows only — offer a hover "+" to add another. Absolutely positioned over
  // the SAME space the grid already leaves between rows (never a real grid
  // item), so none of this can drift the virtualization math above. Masonry
  // has no shared row boundary across the width (each column flows
  // independently), so it gets none of this.
  const rowGaps = $derived.by(() => {
    if (layout === "masonry") return [];
    const set = new Set(gridProseByRow.keys());
    if (totalRows >= 2) {
      const last = Math.min(lastRow, totalRows - 2);
      for (let r = firstRow; r <= last; r++) set.add(r);
    }
    return [...set].sort((a, b) => a - b);
  });
  /** @param {number} r */
  const canAddAt = (r) => r >= firstRow && r <= Math.min(lastRow, totalRows - 2);

  const HOVER_DWELL_MS = 900;
  /** @type {number | null} */
  let dwellingRow = $state(null);
  /** @type {number | null} */
  let editingRow = $state(null);
  /** @type {string | null} */
  let editingBlockId = $state(null); // null = composing a new paragraph
  let draftText = $state("");
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let dwellTimer;

  /** @param {number} row */
  function armDwell(row) {
    if (editingRow !== null || !canAddAt(row)) return; // don't fight an open composer
    clearTimeout(dwellTimer);
    dwellTimer = setTimeout(() => (dwellingRow = row), HOVER_DWELL_MS);
  }
  /** @param {number} row */
  function disarmDwell(row) {
    clearTimeout(dwellTimer);
    if (dwellingRow === row) dwellingRow = null;
  }

  /**
   * @param {number} row
   * @param {{id: string, text: string}} [existing] editing this paragraph instead of adding a new one
   */
  function openComposer(row, existing) {
    clearTimeout(dwellTimer);
    editingRow = row;
    editingBlockId = existing?.id ?? null;
    draftText = existing?.text ?? "";
  }
  function closeComposer() {
    editingRow = null;
    editingBlockId = null;
    dwellingRow = null;
    draftText = "";
  }
  async function submitComposer() {
    const row = editingRow;
    const blockId = editingBlockId;
    const text = draftText;
    closeComposer();
    if (row === null) return;
    if (blockId || text.trim()) await onSaveProse(row, text, blockId ?? undefined);
  }
  /** @param {KeyboardEvent} e */
  function onComposerKey(e) {
    if (e.key === "Escape") {
      e.stopPropagation();
      closeComposer();
    } else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      e.stopPropagation();
      submitComposer();
    }
  }

  let isProgrammaticScroll = false;

  // Parent-driven scroll restore (per-folder memory).
  $effect(() => {
    const limit = virtual ? Math.max(0, totalRows * rowPitch + 64 + inset - safeViewH) : Infinity;
    const target = Math.max(0, Math.min(scrollTop, limit));
    if (gridElement && (Math.abs(gridElement.scrollTop - target) > 2 || top !== target)) {
      isProgrammaticScroll = true;
      top = target;
      gridElement.scrollTop = target;
      // Report a clamp only once the geometry is real. While `frames` is
      // still empty `totalRows` is 0, so `limit` computes to 0 and every
      // restored position clamps to the top — reporting THAT wrote 0 back
      // over the offset the parent had just read from storage, which is why
      // reopening a folder always landed at the top however far you had
      // scrolled. An empty grid cannot clamp anything.
      if (target !== scrollTop && frames.length) untrack(() => onScroll(target));
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
      const y0 = inset + row * rowPitch;
      const y1 = y0 + rowH + gap;
      const viewportHeight = safeViewH;
      const st = element.scrollTop;
      // The top `inset` of the viewport is under the bar: a row there is not visible.
      const target = y0 < st + inset ? y0 - inset : y1 > st + viewportHeight ? y1 - viewportHeight : null;
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
</script>

<div
  class="grid-wrapper"
  bind:clientWidth={viewW}
  bind:clientHeight={viewH}
  bind:this={gridElement}
  onscroll={handleScroll}
  style="scroll-padding-top: {inset}px"
>
  <div
    class="photo-grid"
    class:masonry={layout === "masonry"}
    style="--cols: {cols}; --gap: {gap}px; --cellw: {cellW}px; --cellW: {cellW}px; padding-top: {gap / 2 +
      padTop + inset}px; padding-bottom: {64 + padBottom}px"
  >
    {#snippet cell(/** @type {PhotoFrame} */ f, /** @type {number} */ i)}
      <PhotoCell
        path={f.path}
        name={f.name}
        previewVersion={f.previewVersion}
        rating={f.rating}
        knownAspect={f.width && f.height ? f.width / f.height : undefined}
        selected={selectedPaths.has(f.path)}
        inStory={storySet.has(stem(f.name))}
        isExporting={isExporting(f)}
        {layout}
        {aspect}
        {fill}
        idx={i}
        onSelect={(/** @type {MouseEvent | undefined} */ event) => onSelect(i, event)}
        onDblClick={() => onDblClick(f.path)}
        onContextMenu={(/** @type {MouseEvent} */ event) => onContextMenu(i, event)}
        onToggleStory={() => onToggleStory(f.path)}
        onDragStart={(/** @type {DragEvent} */ event) => onDragStart(f.path, event)}
      />
    {/snippet}

    {#if layout === "masonry"}
      {#each masonryColumns as column, c (c)}
        <div class="masonry-col">
          {#each column as i (frames[i].path)}
            {@render cell(frames[i], i)}
          {/each}
        </div>
      {/each}
    {:else}
      {#each slice as f, j (f.path)}
        {@render cell(f, start + j)}
      {/each}
    {/if}

    {#each rowGaps as r (r)}
      {@const center = r === -1 ? gap / 4 : gap / 2 + padTop + (r - firstRow) * rowPitch + rowH + gap / 2}
      {@const hitH = r === -1 ? gap / 2 : Math.max(gap, 10)}
      {@const prose = gridProseByRow.get(r) ?? []}
      {@const expanded = editingRow === r || prose.length > 0}
      <div
        class="row-gap"
        role="presentation"
        class:active={dwellingRow === r || expanded}
        class:expanded
        style="top: {expanded ? center - 14 : center - hitH / 2}px; height: {expanded ? 'auto' : hitH + 'px'};"
        onmouseenter={() => armDwell(r)}
        onmouseleave={() => disarmDwell(r)}
      >
        {#if editingRow === r}
          <div class="row-gap-composer" role="presentation" onclick={(e) => e.stopPropagation()}>
            <textarea
              rows="1"
              placeholder="Add a paragraph…"
              bind:value={draftText}
              onkeydown={onComposerKey}
              onblur={() => (draftText.trim() ? submitComposer() : closeComposer())}
              use:autofocus
            ></textarea>
          </div>
        {:else}
          {#if prose.length}
            <div class="row-gap-prose">
              {#each prose as p (p.id)}
                <button class="row-gap-text" onclick={() => openComposer(r, p)} title="Edit this paragraph">
                  {p.text}
                </button>
              {/each}
            </div>
          {/if}
          {#if dwellingRow === r && canAddAt(r)}
            <button
              class="row-gap-add"
              title="Add a paragraph here"
              onclick={() => openComposer(r)}
            >
              <Icon name="plus" size="var(--icon-sm)" />
            </button>
          {/if}
        {/if}
      </div>
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
    position: relative;
    display: grid;
    grid-template-columns: repeat(var(--cols), var(--cellw));
    grid-auto-flow: row;
    grid-auto-columns: unset;
    justify-content: center;
    gap: var(--gap);
    padding-left: var(--gap);
    padding-right: var(--gap);
  }

  /* Lives INSIDE the gap the grid already leaves between rows — never a real
     grid item, so it can't push rows apart or drift the virtualized scroll
     math. Invisible until a ~1s dwell proves you meant to stop there, so a
     fast cull pass never sees it flicker. */
  .row-gap {
    position: absolute;
    left: var(--gap);
    right: var(--gap);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-d4);
    z-index: 2;
  }
  .row-gap.expanded {
    z-index: 3;
  }
  .row-gap-add {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    cursor: pointer;
    opacity: 0;
    animation: row-gap-in 0.15s ease forwards;
  }
  @keyframes row-gap-in {
    to { opacity: 1; }
  }
  .row-gap-prose {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
    width: 100%;
  }
  .row-gap-text {
    display: block;
    width: 100%;
    text-align: left;
    padding: var(--space-d3) var(--space-d2);
    cursor: pointer;
    white-space: pre-wrap;
  }
  .row-gap-composer {
    width: 100%;
    padding: var(--space-d8) 0;
  }
  .row-gap-composer textarea {
    width: 100%;
    resize: none;
    font: var(--font-text);
    padding: var(--space-d3) var(--space-d2);
  }

  /* Masonry — columns are real elements, filled by masonryColumns above. */
  .photo-grid.masonry {
    display: flex;
    align-items: flex-start;
    gap: var(--gap);
  }
  .masonry-col {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--gap);
  }
  .photo-grid.masonry :global(.cell) {
    width: 100%;
    /* The uniform grid's shadow-isolation transform is not needed here
       (there are no centered tracks to smear a shadow across). */
    transform: none;
    isolation: auto;
  }
</style>
