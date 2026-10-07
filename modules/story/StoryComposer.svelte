<script>
  // The Story composer — a WYSIWYG port of the Swift compose surface
  // (CullView.swift `StoryPhotoRow`/`StoryPhotoCell`): the note is shown as it
  // will read on the Garden — photos in justified rows, captions in place,
  // prose between — and edited directly. No markdown editor; the block model
  // (lib/story.js) keeps the file a plain hand-editable note underneath.
  /** @import { Block, DropPlace } from "./storyParser.js" */
  /** @typedef {ReturnType<typeof storyRows>[number]} StoryRow */
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri, Icon } from "@modules/core";
  import { parseStory, serializeStory, storyRows, stemOf, settleRows, removeBlock, moveBlock, moveBlockBefore } from "./storyParser.js";

  // Subcomponents
  import StoryFilmRoll from "./StoryFilmRoll.svelte";
  import StoryProseRow from "./StoryProseRow.svelte";
  import StoryPhotoRow from "./StoryPhotoRow.svelte";
  import StoryGap from "./StoryGap.svelte";

  /**
   * A photo frame shown in the film roll / used for thumbnails.
   * @typedef {{ name: string, path: string, previewVersion?: number, capture_at?: number | string }} Frame
   */

  /**
   * @typedef {Object} Props
   * @property {string} [content]
   * @property {Frame[]} [frames]
   * @property {Set<string>} [storySet]
   * @property {(path: string, version: number) => string} thumbUrl
   * @property {((content: string) => void) | Function} [onSave]
   */

  /** @type {Props} */
  let {
    content = "",
    frames = [],
    storySet = new Set(),
    thumbUrl,
    onSave = () => {},
  } = $props();

  /** @param {string} n */
  const stem = (n) => stemOf(n);

  /** @type {string} */
  let frontmatter = $state("");
  /** @type {Block[]} */
  let blocks = $state([]);
  /** @type {Record<string, number>} */
  let aspects = $state({}); // stem → aspect ratio (from <img> onload; 1.5 until known)
  let containerW = $state(900);
  /** @type {string | null} */
  let dragId = $state(null); // block id being dragged
  /** @type {{ id: string, place: DropPlace } | null} */
  let dropTarget = $state(null); // photo cell currently a drag target
  /** @type {number | null} */
  let dropGap = $state(null); // gap index currently a break target

  // Re-init blocks whenever the note content prop changes
  $effect(() => {
    const p = parseStory(content);
    untrack(() => {
      frontmatter = p.frontmatter;
      blocks = p.blocks;
    });
  });

  const rows = $derived(storyRows(blocks));
  const frameByStem = $derived(new Map(frames.map((f) => [stemOf(f.name), f])));

  /** @param {string} p */
  const uid = (p) => `${p}${Math.random().toString(36).slice(2, 9)}`;
  /** @param {string} s */
  const aspectOf = (s) => aspects[s] ?? 1.5;

  /**
   * @param {string} s
   * @param {Event} e
   */
  function onImgLoad(s, e) {
    const img = /** @type {HTMLImageElement} */ (e.currentTarget);
    const w = img.naturalWidth;
    const h = img.naturalHeight;
    if (w && h) aspects = { ...aspects, [s]: w / h };
  }

  // Swift's justified row: shared height so the run fills the column width,
  // clamped 90–320 so a lone portrait or a long strip stays composed.
  /** @param {Block[]} rowBlocks */
  function rowLayout(rowBlocks) {
    const gap = 8;
    const avail = Math.max(containerW - gap * (rowBlocks.length - 1), 1);
    const sumA = Math.max(rowBlocks.reduce((s, b) => s + aspectOf(b.stem), 0), 0.0001);
    const h = Math.min(Math.max(avail / sumA, 90), 320);
    return rowBlocks.map((b) => ({ block: b, w: h * aspectOf(b.stem), h }));
  }

  // ---- persistence -------------------------------------------------------
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let saveTimer;
  function commit(now = false) {
    const c = serializeStory(frontmatter, blocks);
    clearTimeout(saveTimer);
    if (now) onSave(c);
    else saveTimer = setTimeout(() => onSave(c), 300);
  }

  /** @type {Record<string, ReturnType<typeof setTimeout>>} */
  const captionSaveTimers = {};
  /** @param {string} stem @param {string} text */
  function syncCaptionToSidecar(stem, text) {
    if (!isTauri) return;
    const f = frameByStem.get(stem);
    if (!f) return;
    clearTimeout(captionSaveTimers[stem]);
    captionSaveTimers[stem] = setTimeout(() => {
      invoke("save_caption", { path: f.path, description: text }).catch(() => {});
    }, 300);
  }

  // ---- edits -------------------------------------------------------------
  /**
   * @param {string} id
   * @param {string} text
   */
  function setText(id, text) {
    const b = blocks.find((x) => x.id === id);
    if (!b) return;
    if (b.isPhoto) syncCaptionToSidecar(b.stem, text);
    b.text = text;
    blocks = [...blocks];
    commit();
  }

  /** @param {string} id */
  function remove(id) {
    blocks = removeBlock(blocks, id);
    commit(true);
  }

  /** @param {string} name */
  function addPhoto(name) {
    const s = stem(name);
    if (blocks.some((b) => b.isPhoto && b.stem === s)) return; // already in
    blocks = [...blocks, { id: uid("ph"), isPhoto: true, stem: s, text: "", rowBreak: true }];
    normalize();
    commit(true);
  }

  /**
   * @param {string} stemName
   * @returns {string} the id to move — the existing block's when it is already in
   */
  function ensurePhoto(stemName) {
    const s = stem(stemName);
    const existing = blocks.find((b) => b.isPhoto && b.stem === s);
    if (existing) return existing.id;
    const block = { id: uid("ph"), isPhoto: true, stem: s, text: "", rowBreak: true };
    blocks = [...blocks, block];
    return block.id;
  }

  const proseBlock = () => ({ id: uid("pr"), isPhoto: false, stem: "", text: "", rowBreak: true });

  function addProse() {
    blocks = [...blocks, proseBlock()];
  }

  /// The block index where render-row `r` starts (`rows.length` → the end).
  /** @param {number} r */
  function blockIndexOfRow(r) {
    if (r >= rows.length) return blocks.length;
    const row = rows[r];
    const firstId = row.type === "prose" ? row.block.id : row.blocks[0].id;
    const i = blocks.findIndex((b) => b.id === firstId);
    return i < 0 ? blocks.length : i;
  }

  /** @param {number} r */
  function addProseAt(r) {
    const at = blockIndexOfRow(r);
    blocks = [...blocks.slice(0, at), proseBlock(), ...blocks.slice(at)];
    normalize();
  }

  function normalize() {
    blocks = settleRows(blocks);
  }

  // ---- drag & drop -------------------------------------------------------
  /**
   * @param {string} id
   * @param {string | null} beforeId
   */
  function moveBefore(id, beforeId) {
    blocks = moveBlockBefore(blocks, id, beforeId);
    commit(true);
  }

  /**
   * @param {number} fromRowIdx
   * @param {number} toRowIdx
   */
  function moveRow(fromRowIdx, toRowIdx) {
    if (fromRowIdx === toRowIdx || fromRowIdx < 0 || fromRowIdx >= rows.length || toRowIdx < 0 || toRowIdx >= rows.length) return;
    const sourceRow = rows[fromRowIdx];
    const targetRow = rows[toRowIdx];
    const sourceBlockId = sourceRow.type === "prose" ? sourceRow.block.id : sourceRow.blocks[0].id;
    let beforeId = null;
    if (toRowIdx > fromRowIdx) {
      if (toRowIdx + 1 < rows.length) {
        const nextRow = rows[toRowIdx + 1];
        beforeId = nextRow.type === "prose" ? nextRow.block.id : nextRow.blocks[0].id;
      } else {
        beforeId = null;
      }
    } else {
      beforeId = targetRow.type === "prose" ? targetRow.block.id : targetRow.blocks[0].id;
    }
    moveBefore(sourceBlockId, beforeId);
  }

  /** @param {string} id */
  function breakOut(id) {
    const i = blocks.findIndex((b) => b.id === id);
    if (i < 0) return;
    blocks[i].rowBreak = true;
    normalize();
    commit(true);
  }

  function splitAll() {
    for (const b of blocks) {
      if (b.isPhoto) b.rowBreak = true;
    }
    normalize();
    commit(true);
  }

  function sortByDate() {
    /** @type {Array<{ photoRow: StoryRow | null, trailing: StoryRow[] }>} */
    const units = [];
    /** @type {{ photoRow: StoryRow | null, trailing: StoryRow[] } | null} */
    let current = null;
    for (const row of rows) {
      if (row.type === "photos") {
        current = { photoRow: row, trailing: [] };
        units.push(current);
      } else if (current) {
        current.trailing.push(row);
      } else {
        units.push({ photoRow: null, trailing: [row] });
      }
    }

    /** @param {StoryRow} row */
    const earliestCapture = (row) => {
      if (row.type !== "photos") return 0;
      const times = row.blocks
        .map((b) => Number(frameByStem.get(b.stem)?.capture_at) || 0)
        .filter((t) => t > 0);
      return times.length ? Math.min(...times) : Infinity;
    };

    const fixed = units.filter((u) => !u.photoRow);
    const sortable = units.filter((u) => /** @type {StoryRow} */ (u.photoRow));
    sortable.sort(
      (a, b) =>
        earliestCapture(/** @type {StoryRow} */ (a.photoRow)) -
        earliestCapture(/** @type {StoryRow} */ (b.photoRow)),
    );

    /** @type {Block[]} */
    const newBlocks = [];
    for (const u of [...fixed, ...sortable]) {
      if (u.photoRow?.type === "photos") newBlocks.push(...u.photoRow.blocks);
      for (const r of u.trailing) {
        if (r.type === "prose") newBlocks.push(r.block);
      }
    }
    blocks = newBlocks;
    normalize();
    commit(true);
  }

  /**
   * @param {DragEvent} e
   * @param {string} cellId
   */
  function onCellDragOver(e, cellId) {
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    if (dragId === cellId) return;

    const rect = /** @type {HTMLElement} */ (e.currentTarget).getBoundingClientRect();
    const relX = (e.clientX - rect.left) / (rect.width || 1);
    const relY = (e.clientY - rect.top) / (rect.height || 1);

    /** @type {DropPlace} */
    let place;
    if (relX < 0.3) place = "left";
    else if (relX > 0.7) place = "right";
    else place = relY < 0.5 ? "above" : "below";

    dropTarget = { id: cellId, place };
    dropGap = null;
  }

  /**
   * @param {DragEvent} e
   * @param {string} targetId
   */
  function onCellDrop(e, targetId) {
    e.preventDefault();
    e.stopPropagation();
    const dragged = dragId || e.dataTransfer?.getData("text/plain");
    if (!dragged || dragged === targetId) {
      onDragEnd();
      return;
    }
    const place = dropTarget?.id === targetId ? dropTarget.place : "right";
    const id = blocks.some((b) => b.id === dragged) ? dragged : ensurePhoto(dragged);
    blocks = moveBlock(blocks, id, targetId, place);
    commit(true);
    onDragEnd();
  }

  /**
   * @param {string} id
   * @param {DragEvent} e
   */
  function onDragStart(id, e) {
    dragId = id;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", id);

      const target = /** @type {HTMLElement} */ (e.currentTarget);
      const img = target?.querySelector?.("img");
      if (img instanceof HTMLImageElement && e.dataTransfer.setDragImage) {
        e.dataTransfer.setDragImage(img, Math.round(img.offsetWidth / 2), Math.round(img.offsetHeight / 2));
      } else if (e.dataTransfer.setDragImage) {
        const rowEl = target.closest(".prose-row");
        if (rowEl instanceof HTMLElement) {
          e.dataTransfer.setDragImage(rowEl, 40, 20);
        }
      }
    }
  }

  /**
   * @param {number} clientY
   * @param {HTMLElement} host
   */
  function resolveDropGap(clientY, host) {
    const rowEls = host.querySelectorAll("[data-row-first]");
    for (let i = 0; i < rowEls.length; i++) {
      const r = rowEls[i].getBoundingClientRect();
      if (clientY < r.top + r.height / 2) return i;
    }
    return rowEls.length;
  }

  /** @param {number} gap */
  function beforeIdForGap(gap) {
    return gap < rows.length ? (rows[gap].type === "prose" ? rows[gap].block.id : rows[gap].blocks[0].id) : null;
  }

  /** @param {DragEvent} e */
  function onSurfaceDragOver(e) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    dropTarget = null;
    dropGap = resolveDropGap(e.clientY, /** @type {HTMLElement} */ (e.currentTarget));
  }

  /** @param {DragEvent} e */
  function onSurfaceDrop(e) {
    e.preventDefault();
    const id = dragId || e.dataTransfer?.getData("text/plain");
    if (!id) {
      onDragEnd();
      return;
    }
    const gap = resolveDropGap(e.clientY, /** @type {HTMLElement} */ (e.currentTarget));
    const beforeId = beforeIdForGap(gap);
    moveBefore(blocks.some((b) => b.id === id) ? id : ensurePhoto(id), beforeId);
    onDragEnd();
  }

  function onDragEnd() {
    dragId = null;
    dropTarget = null;
    dropGap = null;
  }
</script>

<div class="composer" class:dragging={dragId !== null}>
  <div
    class="surface"
    role="list"
    ondragenter={onSurfaceDragOver}
    ondragover={onSurfaceDragOver}
    ondrop={onSurfaceDrop}
  >
    <div class="measure" bind:clientWidth={containerW}></div>
    {#if !blocks.length}
      <div class="empty">
        <Icon name="stack-simple" size="32px" />
        <p>Drag or click a photo from the film roll to start the story.</p>
      </div>
    {:else if blocks.filter((b) => b.isPhoto).length > 1}
      <div class="row-tools">
        {#if rows.some((r) => r.type === "photos" && r.blocks.length > 1)}
          <button class="split-all-btn" onclick={splitAll} title="Place each photo on its own row">
            <Icon name="rows" size="var(--icon-md)" /><span>Split into individual rows</span>
          </button>
        {/if}
        <button class="split-all-btn" onclick={sortByDate} title="Reorder photos chronologically">
          <Icon name="arrows-down-up" size="var(--icon-md)" /><span>By date</span>
        </button>
      </div>
    {/if}

    {#each rows as row, r (row.type === "prose" ? row.block.id : row.blocks[0].id)}
      <StoryGap
        rowIndex={r}
        isFirst={r === 0}
        isOver={dropGap === r}
        onDragEnter={(e, idx) => {
          e.preventDefault();
          e.stopPropagation();
          if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
          dropGap = idx;
        }}
        onDragOver={(e, idx) => {
          e.preventDefault();
          e.stopPropagation();
          if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
          dropGap = idx;
        }}
        onDragLeave={(idx) => dropGap === idx && (dropGap = null)}
        onDrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          const before = row.type === "prose" ? row.block.id : row.blocks[0].id;
          const id = dragId || e.dataTransfer?.getData("text/plain");
          if (id) {
            moveBefore(blocks.some((b) => b.id === id) ? id : ensurePhoto(id), before);
          }
          onDragEnd();
        }}
        onAddProseAt={addProseAt}
      />

      {#if row.type === "prose"}
        <StoryProseRow
          block={row.block}
          rowIndex={r}
          totalRows={rows.length}
          isDragging={dragId === row.block.id}
          onTextChange={setText}
          onMoveRow={moveRow}
          onRemove={remove}
          {onDragStart}
          {onDragEnd}
        />
      {:else}
        <StoryPhotoRow
          {row}
          {dragId}
          {dropTarget}
          {frameByStem}
          {thumbUrl}
          {rowLayout}
          {onImgLoad}
          {onDragStart}
          {onDragEnd}
          {onCellDragOver}
          {onCellDrop}
          onBreakOut={breakOut}
          onRemove={remove}
          onSetText={setText}
          onClearDropTarget={() => (dropTarget = null)}
        />
      {/if}
    {/each}

    <StoryGap
      rowIndex={rows.length}
      isEnd={true}
      isOver={dropGap === rows.length}
      onDragEnter={(e, idx) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
        dropGap = idx;
      }}
      onDragOver={(e, idx) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
        dropGap = idx;
      }}
      onDragLeave={(idx) => dropGap === idx && (dropGap = null)}
      onDrop={(e) => {
        e.preventDefault();
        e.stopPropagation();
        const id = dragId || e.dataTransfer?.getData("text/plain");
        if (id) moveBefore(blocks.some((b) => b.id === id) ? id : ensurePhoto(id), null);
        onDragEnd();
      }}
    />

    {#if blocks.length}
      <button class="add-prose" onclick={addProse}>
        <Icon name="plus" size="var(--icon-md)" />
        <span>Add a paragraph</span>
      </button>
    {/if}
  </div>

  <StoryFilmRoll
    {frames}
    {storySet}
    {thumbUrl}
    onAddPhoto={addPhoto}
    {onDragStart}
    {onDragEnd}
  />
</div>

<style>
  .composer {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transition: background var(--duration-slow) var(--ease-standard);
  }

  .surface {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    padding: calc(var(--space-d4) * 6) max(24px, 5vw) calc(var(--space-d4) * 31);
    display: flex;
    flex-direction: column;
    max-width: 1100px;
    width: 100%;
    margin: 0 auto;
    box-sizing: border-box;
    color: var(--color-foreground);
    transition: all var(--transition-fast);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space);
    text-align: center;
    padding: calc(var(--space-d4) * 21) var(--space);
    opacity: 0.6;
  }
  .empty p {
    margin: 0;
    max-width: 24rem;
  }

  .row-tools {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-d2);
    margin-bottom: calc(var(--space-d4) * 5);
  }
  .split-all-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d3);
    padding: var(--space-d3) calc(var(--space-d4) * 3);
    cursor: pointer;
  }

  .measure {
    width: 100%;
    height: 0;
    flex-shrink: 0;
    pointer-events: none;
  }

  .composer.dragging :global(.caption),
  .composer.dragging :global(.cell-remove),
  .composer.dragging :global(.cell-break),
  .composer.dragging :global(.prose),
  .composer.dragging :global(.row-grip),
  .composer.dragging :global(.row-side-actions),
  .composer.dragging :global(.add-prose),
  .composer.dragging :global(.row-tools) {
    pointer-events: none;
  }

  .add-prose {
    align-self: center;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--space-d2);
    margin-top: calc(var(--space-d4) * 6);
    padding: calc(var(--space-d4) * 3) calc(var(--space-d4) * 6);
  }
</style>
