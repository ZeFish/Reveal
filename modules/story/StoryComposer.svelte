<script>
  // The Story composer — a WYSIWYG port of the Swift compose surface
  // (CullView.swift `StoryPhotoRow`/`StoryPhotoCell`): the note is shown as it
  // will read on the Garden — photos in justified rows, captions in place,
  // prose between — and edited directly. No markdown editor; the block model
  // (lib/story.js) keeps the file a plain hand-editable note underneath.
  /** @import { Block } from "$lib/story.js" */
  /** @typedef {ReturnType<typeof storyRows>[number]} StoryRow */
  import { untrack } from "svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { parseStory, serializeStory, storyRows, stemOf } from "$lib/story.js";

  /**
   * A photo frame shown in the film roll / used for thumbnails.
   * @typedef {{ name: string, path: string, previewVersion?: number }} Frame
   */
  let {
    content = "", // the note's raw markdown — re-inits the blocks when it changes
    frames = [], // the folder's photos, for thumbnails + aspect ratios
    storySet = new Set(), // stems already in the story (film-roll markers)
    /** @type {(path: string, version: number) => string} */
    thumbUrl, // (path, version) => url
    /** @type {(content: string) => void} */
    onSave = () => {}, // (content) => void  — persist the serialized note
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
  /** @type {{ id: string, mode: "before" | "group" | "after" } | null} */
  let dropTarget = $state(null); // photo cell currently a drag target
  /** @type {number | null} */
  let dropGap = $state(null); // gap index currently a break target

  // Re-init blocks whenever the note content prop changes — the parent only
  // sets it on a real load (opening a folder), never echoing our own saves
  // back, so this fires on folder-open but stays quiet during a session (no
  // id churn, no lost focus while editing a caption).
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

  // ---- edits -------------------------------------------------------------
  /**
   * @param {string} id
   * @param {string} text
   */
  function setText(id, text) {
    const b = blocks.find((x) => x.id === id);
    if (!b) return;
    b.text = text;
    blocks = [...blocks];
    commit();
  }
  /** @param {string} id */
  function remove(id) {
    blocks = blocks.filter((x) => x.id !== id);
    normalize();
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
   * Insert a photo stem from the film roll into a specific position as a new row.
   * @param {string} stemName
   * @param {string | null} beforeId
   */
  function insertPhotoAt(stemName, beforeId) {
    const s = stem(stemName);
    const existingIndex = blocks.findIndex((b) => b.isPhoto && b.stem === s);
    if (existingIndex >= 0) {
      moveBefore(blocks[existingIndex].id, beforeId);
      return;
    }
    const newBlock = { id: uid("ph"), isPhoto: true, stem: s, text: "", rowBreak: true };
    const at = beforeId ? blocks.findIndex((b) => b.id === beforeId) : blocks.length;
    const targetIdx = at < 0 ? blocks.length : at;
    blocks = [...blocks.slice(0, targetIdx), newBlock, ...blocks.slice(targetIdx)];
    normalize();
    commit(true);
  }

  /**
   * Group a photo stem from the film roll onto an existing photo cell (side-by-side).
   * @param {string} stemName
   * @param {string} targetId
   */
  function groupPhotoOnto(stemName, targetId) {
    const s = stem(stemName);
    const existingIndex = blocks.findIndex((b) => b.isPhoto && b.stem === s);
    if (existingIndex >= 0) {
      groupOnto(blocks[existingIndex].id, targetId);
      return;
    }
    const newBlock = { id: uid("ph"), isPhoto: true, stem: s, text: "", rowBreak: false };
    const to = blocks.findIndex((b) => b.id === targetId);
    if (to < 0) {
      blocks = [...blocks, newBlock];
    } else {
      blocks = [...blocks.slice(0, to + 1), newBlock, ...blocks.slice(to + 1)];
    }
    normalize();
    commit(true);
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

  /// Insert a paragraph *between* rows — the gap's own affordance, so text can
  /// land between two photos rather than only at the end.
  /** @param {number} r */
  function addProseAt(r) {
    const at = blockIndexOfRow(r);
    blocks = [...blocks.slice(0, at), proseBlock(), ...blocks.slice(at)];
    normalize();
  }

  // A photo joins the row above only when the block before it is a captionless
  // photo — otherwise it must start its own row (Swift `normalizeRowBreaks`).
  function normalize() {
    for (let i = 0; i < blocks.length; i++) {
      if (!blocks[i].isPhoto) continue;
      const prev = i > 0 ? blocks[i - 1] : null;
      const canJoin = prev && prev.isPhoto && (prev.text ?? "").trim() === "";
      if (!canJoin) blocks[i].rowBreak = true;
    }
    blocks = [...blocks];
  }

  // ---- drag: group onto a photo, or break out at a gap -------------------
  /**
   * @param {string} id
   * @param {string | null} beforeId
   */
  function moveBefore(id, beforeId) {
    const from = blocks.findIndex((b) => b.id === id);
    if (from < 0) return;
    const [moved] = blocks.splice(from, 1);
    if (moved.isPhoto) moved.rowBreak = true; // its own row
    let to = beforeId ? blocks.findIndex((b) => b.id === beforeId) : blocks.length;
    if (to < 0) to = blocks.length;
    blocks.splice(to, 0, moved);
    normalize();
    commit(true);
  }
  /**
   * @param {string} id
   * @param {string} targetId
   */
  function groupOnto(id, targetId) {
    if (id === targetId) return;
    const from = blocks.findIndex((b) => b.id === id);
    if (from < 0 || !blocks[from].isPhoto) return;
    const [moved] = blocks.splice(from, 1);
    moved.rowBreak = false; // join the target's row
    const to = blocks.findIndex((b) => b.id === targetId);
    blocks.splice(to < 0 ? blocks.length : to + 1, 0, moved);
    normalize();
    commit(true);
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

  /// Re-order the story chronologically by capture date. A photo *row*
  /// (grouped side-by-side photos share rowBreak=false) moves as one unit,
  /// keyed by its earliest capture_at; a caption/paragraph travels with
  /// whichever row precedes it, since it's usually written about that row.
  /// Leading prose with nothing before it stays fixed at the very top.
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
      return times.length ? Math.min(...times) : Infinity; // frames off-disk sort last
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
    e.stopPropagation(); // this cell owns the preview precisely; don't let .surface's coarser one override it
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    if (dragId === cellId) return;

    const el = /** @type {HTMLElement} */ (e.currentTarget);
    const rect = el.getBoundingClientRect();
    const relY = (e.clientY - rect.top) / (rect.height || 1);

    /** @type {"group" | "before" | "after"} */
    let mode = "group";
    if (relY < 0.28) {
      mode = "before";
    } else if (relY > 0.72) {
      mode = "after";
    }

    dropTarget = { id: cellId, mode };
    dropGap = null;
  }

  /**
   * @param {DragEvent} e
   * @param {string} targetId
   */
  function onCellDrop(e, targetId) {
    e.preventDefault();
    e.stopPropagation(); // handled precisely here; don't let .surface re-handle it
    const id = dragId || e.dataTransfer?.getData("text/plain");
    if (!id || id === targetId) {
      onDragEnd();
      return;
    }

    const mode = dropTarget?.id === targetId ? dropTarget.mode : "group";
    const isExistingBlock = blocks.some((b) => b.id === id);

    if (mode === "before") {
      if (isExistingBlock) {
        moveBefore(id, targetId);
      } else {
        insertPhotoAt(id, targetId);
      }
    } else if (mode === "after") {
      const idx = blocks.findIndex((b) => b.id === targetId);
      const nextId = idx >= 0 && idx + 1 < blocks.length ? blocks[idx + 1].id : null;
      if (isExistingBlock) {
        moveBefore(id, nextId);
      } else {
        insertPhotoAt(id, nextId);
      }
    } else {
      if (isExistingBlock) {
        groupOnto(id, targetId);
      } else {
        groupPhotoOnto(id, targetId);
      }
    }
    onDragEnd();
  }

  /**
   * @param {string} id
   * @param {DragEvent} e
   */
  function onDragStart(id, e) {
    // NOTE: deliberately NOT resetting the counters here. Resetting per-drag
    // erased the evidence of the previous release, and every screenshot so
    // far was taken mid-flight. Cumulative totals answer the only question
    // that matters: does `drop`/`dragend` EVER fire, across any attempt?
    dragId = id;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", id);

      // Without this, WebKit's default drag image is a full screenshot of
      // the dragged element (cell chrome, buttons and all) — reads as a
      // generic "you're dragging a webpage image out" ghost. A cropped,
      // cursor-centred thumbnail reads as picking up a photo card instead.
      const target = /** @type {HTMLElement} */ (e.currentTarget);
      const img = target?.querySelector?.("img");
      if (img instanceof HTMLImageElement && e.dataTransfer.setDragImage) {
        e.dataTransfer.setDragImage(img, Math.round(img.offsetWidth / 2), Math.round(img.offsetHeight / 2));
      }
    }
  }

  /**
   * The row index a clientY falls before, by comparing against each rendered
   * row's midpoint — the same convention as `dropGap` (`rows.length` means
   * "after everything", matching `.gap.end`). Shared by the live preview and
   * the actual drop so they never disagree about where the photo will land.
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

  /**
   * Live preview for the composition surface. The precise zones (.gap,
   * .photo-cell) stopPropagation on dragenter/dragover, so this only runs
   * over dead space — the flanks of a centred .photo-row, the margins
   * between rows, .surface's own padding, a prose row. Those areas dominate
   * the surface, and because +page.svelte preventDefaults `dragover` on
   * window, hovering them never showed a "no-drop" cursor: there was no way
   * to tell a miss from a hit until release, which is exactly why this read
   * as "drag and drop is broken" rather than "you're 20px off target".
   * @param {DragEvent} e
   */
  function onSurfaceDragOver(e) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    dropTarget = null; // a precise cell target isn't under the pointer here
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
    if (blocks.some((b) => b.id === id)) moveBefore(id, beforeId);
    else insertPhotoAt(id, beforeId);
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
    <!-- Zero-height sentinel: rowLayout must size rows against the CONTENT box
         the cells actually live in, not the outer shell. Measuring .composer
         overstated the width by .surface's horizontal padding
         (2 x max(24px, 4vw)) plus anything past its max-width:1100px, so
         multi-photo rows were computed too wide, overflowed a
         justify-content:center row, and their leftmost cell was pushed into
         unreachable negative overflow. -->
    <div class="measure" bind:clientWidth={containerW}></div>
    {#if !blocks.length}
      <div class="empty">Clique une photo de la pellicule pour commencer l'histoire.</div>
    {:else if blocks.filter((b) => b.isPhoto).length > 1}
      <div class="row-tools">
        {#if rows.some((r) => r.type === "photos" && r.blocks.length > 1)}
          <button class="split-all-btn" onclick={splitAll} title="Placer chaque photo sur sa propre ligne">
            <Icon name="rows" size="11px" /><span>Séparer toutes les photos en lignes</span>
          </button>
        {/if}
        <button class="split-all-btn" onclick={sortByDate} title="Réordonner les photos par date de prise de vue">
          <Icon name="arrows-down-up" size="11px" /><span>Trier par date</span>
        </button>
      </div>
    {/if}

    {#each rows as row, r (row.type === "prose" ? row.block.id : row.blocks[0].id)}
      <!-- Break-out drop zone before each row: dropping a photo here gives it
           its own row at this position. -->
      <div
        class="gap"
        class:over={dropGap === r}
        role="separator"
        ondragenter={(e) => {
          e.preventDefault();
          e.stopPropagation();
          if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
          dropGap = r;
        }}
        ondragover={(e) => {
          e.preventDefault();
          e.stopPropagation();
          if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
          dropGap = r;
        }}
        ondragleave={() => dropGap === r && (dropGap = null)}
        ondrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          const before = row.type === "prose" ? row.block.id : row.blocks[0].id;
          const id = dragId || e.dataTransfer?.getData("text/plain");
          if (id) {
            const isExistingBlock = blocks.some((b) => b.id === id);
            if (isExistingBlock) {
              moveBefore(id, before);
            } else {
              insertPhotoAt(id, before);
            }
          }
          onDragEnd();
        }}
      >
        <!-- Hover the space between two rows to drop a paragraph in there. -->
        <button class="gap-add" onclick={() => addProseAt(r)} title="Insérer un paragraphe ici">
          <Icon name="note-pencil" size="9px" /><span>texte</span>
        </button>
      </div>

      {#if row.type === "prose"}
        <div class="prose-row" data-row-first={row.block.id}>
          <textarea
            class="prose"
            rows="2"
            placeholder="Écris un paragraphe…"
            value={row.block.text}
            oninput={(e) => setText(row.block.id, e.currentTarget.value)}
          ></textarea>
          <button class="row-remove" onclick={() => remove(row.block.id)} title="Retirer ce texte">
            <Icon name="x" size="10px" />
          </button>
        </div>
      {:else}
        <div class="photo-row" style="gap: 8px;" data-row-first={row.blocks[0].id}>
          {#each rowLayout(row.blocks) as cell (cell.block.id)}
            {@const f = frameByStem.get(cell.block.stem)}
            <div
              class="photo-cell"
              class:dragging={dragId === cell.block.id}
              class:drop-group={dropTarget?.id === cell.block.id && dropTarget?.mode === "group"}
              class:drop-before={dropTarget?.id === cell.block.id && dropTarget?.mode === "before"}
              class:drop-after={dropTarget?.id === cell.block.id && dropTarget?.mode === "after"}
              style="width: {cell.w}px;"
              draggable="true"
              role="listitem"
              ondragstart={(e) => onDragStart(cell.block.id, e)}
              ondragend={onDragEnd}
              ondragenter={(e) => onCellDragOver(e, cell.block.id)}
              ondragover={(e) => onCellDragOver(e, cell.block.id)}
              ondragleave={() => dropTarget?.id === cell.block.id && (dropTarget = null)}
              ondrop={(e) => onCellDrop(e, cell.block.id)}
            >
              <div class="frame" style="height: {cell.h}px;">
                {#if f}
                  <img
                    src={thumbUrl(f.path, f.previewVersion ?? 0)}
                    alt={cell.block.stem}
                    draggable="false"
                    onload={(e) => onImgLoad(cell.block.stem, e)}
                  />
                {:else}
                  <div class="missing" title="Photo absente du dossier">{cell.block.stem}</div>
                {/if}
                {#if row.blocks.length > 1}
                  <button class="cell-break" onclick={() => breakOut(cell.block.id)} title="Séparer sur sa propre ligne">
                    <Icon name="rows" size="10px" />
                  </button>
                {/if}
                <button class="cell-remove" onclick={() => remove(cell.block.id)} title="Retirer de l'histoire">
                  <Icon name="x" size="10px" />
                </button>
              </div>
              <input
                class="caption"
                placeholder="légende…"
                value={cell.block.text}
                oninput={(e) => setText(cell.block.id, e.currentTarget.value)}
              />
            </div>
          {/each}
        </div>
      {/if}
    {/each}

    <!-- Trailing gap: drop a photo here to move it to the very end, own row. -->
    <div
      class="gap end"
      class:over={dropGap === rows.length}
      role="separator"
      ondragenter={(e) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
        dropGap = rows.length;
      }}
      ondragover={(e) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
        dropGap = rows.length;
      }}
      ondragleave={() => dropGap === rows.length && (dropGap = null)}
      ondrop={(e) => {
        e.preventDefault();
        e.stopPropagation();
        const id = dragId || e.dataTransfer?.getData("text/plain");
        if (id) {
          const isExistingBlock = blocks.some((b) => b.id === id);
          if (isExistingBlock) {
            moveBefore(id, null);
          } else {
            insertPhotoAt(id, null);
          }
        }
        onDragEnd();
      }}
    ></div>

    {#if blocks.length}
      <button class="add-prose" onclick={addProse}>
        <Icon name="note-pencil" size="11px" /><span>Ajouter du texte à la fin</span>
      </button>
    {/if}
  </div>

  <!-- The film roll sits UNDER the story: the composition is what you're
       reading, the roll is the tray you pick from. Click a frame to append it. -->
  {#if frames.length}
    <div class="roll">
      <span class="roll-title">PELLICULE · {frames.length}</span>
      <div class="roll-strip">
        {#each frames as f (f.path)}
          {@const s = stemOf(f.name)}
          <div
            class="roll-cell"
            class:in-story={storySet.has(s)}
            draggable="true"
            role="button"
            tabindex="0"
            ondragstart={(e) => onDragStart(s, e)}
            ondragend={onDragEnd}
            onclick={() => addPhoto(f.name)}
            onkeydown={(e) => e.key === "Enter" && addPhoto(f.name)}
            title={`Ajouter ${f.name}`}
          >
            <img src={thumbUrl(f.path, f.previewVersion ?? 0)} alt={f.name} loading="lazy" draggable="false" />
            {#if storySet.has(s)}
              <span class="roll-check"><Icon name="check" size="8px" /></span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .composer {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* Film roll — the add surface (same as the old strip). */
  .roll {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
    background: var(--color-surface-low);
    border-top: 1px solid var(--color-border);
  }
  .roll-title {
    flex-shrink: 0;
    font-family: var(--font-header, sans-serif);
    font-size: 0.6rem;
    letter-spacing: 0.1em;
    opacity: 0.55;
    user-select: none;
  }
  .roll-strip {
    display: flex;
    gap: 0.4rem;
    overflow-x: auto;
    padding-bottom: 2px;
  }
  .roll-cell {
    all: unset;
    position: relative;
    flex-shrink: 0;
    width: 56px;
    height: 56px;
    border-radius: var(--radius-sm);
    overflow: hidden;
    cursor: pointer;
    box-shadow: 0 0 0 1px var(--color-border);
  }
  .roll-cell:hover {
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-foreground) 45%, transparent);
  }
  .roll-cell.in-story {
    box-shadow: 0 0 0 2px var(--color-accent);
  }
  .roll-cell img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    /* pointer-events:none is what the WORKING grid cell does
       (PhotoCell.svelte `.matte img`) and what this file was missing. Without
       it the <img> is itself the hit-tested node, so during a drag the target
       flips img -> frame -> cell -> gap and WebKit keeps restarting the drop
       target instead of settling on one. draggable="false" and
       -webkit-user-drag alone do NOT cover this — they stop the native image
       drag, not the hit-testing. */
    pointer-events: none;
    -webkit-user-drag: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .roll-check {
    position: absolute;
    right: 2px;
    bottom: 2px;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background: var(--color-accent);
    color: #fff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  /* The composition surface — the story as it will read. */
  .surface {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    padding: 24px max(24px, 4vw) 140px;
    display: flex;
    flex-direction: column;
    max-width: 1100px;
    width: 100%;
    margin: 0 auto;
    box-sizing: border-box;
  }
  .empty {
    color: var(--color-muted);
    font-family: var(--font-text, sans-serif);
    font-size: 0.9rem;
    text-align: center;
    padding: 3rem 0;
  }

  .photo-row {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    margin: 6px 0;
  }
  .photo-cell {
    position: relative;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    cursor: grab;
  }
  .photo-cell.dragging {
    opacity: 0.4;
  }
  .measure {
    width: 100%;
    height: 0;
    flex-shrink: 0;
    pointer-events: none;
  }
  .frame {
    position: relative;
    width: 100%;
    border-radius: var(--radius-sm);
    overflow: hidden;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
    transition: box-shadow 0.12s var(--ease-standard);
  }
  .photo-cell.drop-group .frame {
    box-shadow: 0 0 0 2px var(--color-accent);
  }
  /* The drop indicators sit OUTSIDE the cell box (negative offsets), so
     without pointer-events:none they enlarge the cell's hit area over the
     neighbouring .gap mid-drag. That changes which node is hit-tested from
     one mouse-move to the next, and WebKit treats a changed target as a
     fresh dragenter rather than a dragover — churning the drop target
     instead of settling on one. Indicators are pure decoration; never let
     them take hits. */
  .photo-cell.drop-before::before {
    content: "";
    position: absolute;
    top: -6px;
    left: -4px;
    right: -4px;
    height: 4px;
    background: var(--color-accent);
    border-radius: var(--radius-sm);
    z-index: 10;
    pointer-events: none;
  }
  .photo-cell.drop-after::after {
    content: "";
    position: absolute;
    bottom: -6px;
    left: -4px;
    right: -4px;
    height: 4px;
    background: var(--color-accent);
    border-radius: var(--radius-sm);
    z-index: 10;
    pointer-events: none;
  }
  .frame img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    /* See .roll-cell img — the image must never be the hit-tested node. */
    pointer-events: none;
    -webkit-user-drag: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .missing {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--color-foreground) 6%, transparent);
    color: var(--color-muted);
    font-family: var(--font-monospace, monospace);
    font-size: 10px;
    text-align: center;
    padding: 4px;
    box-sizing: border-box;
  }
  .cell-remove,
  .row-remove {
    position: absolute;
    top: 6px;
    right: 6px;
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 45%, transparent);
    color: #fff;
    opacity: 0;
    transition: opacity var(--duration-fast) var(--ease-soft);
  }
  .frame:hover .cell-remove,
  .prose-row:hover .row-remove {
    opacity: 1;
  }
  .caption {
    all: unset;
    /* `all: unset` resets display to `inline` — an inline input/textarea sits in
       the line box instead of taking its own block, which made the caption and
       the paragraph below it overlap. Both must be explicit blocks. */
    display: block;
    box-sizing: border-box;
    width: 100%;
    text-align: center;
    font-family: var(--font-text, sans-serif);
    font-size: 11px;
    color: var(--color-muted);
    padding: 2px 4px;
  }
  .caption::placeholder {
    color: color-mix(in srgb, var(--color-foreground) 25%, transparent);
  }
  .caption:focus {
    color: var(--color-foreground);
  }

  .prose-row {
    position: relative;
  }
  .prose {
    all: unset;
    display: block; /* see .caption — `all: unset` would make it inline */
    box-sizing: border-box;
    /* A measure that reads: prose tracks the story's column rather than the
       full pane, and sits centred like the photo rows above it. */
    width: min(100%, 42rem);
    margin: 0 auto;
    font-family: var(--font-text, serif);
    font-size: 1.02rem;
    line-height: 1.6;
    color: var(--color-foreground);
    padding: 10px 0;
    resize: vertical;
    text-align: center;
  }
  .prose::placeholder {
    color: color-mix(in srgb, var(--color-foreground) 30%, transparent);
  }
  .row-remove {
    top: 4px;
    right: 0;
  }

  /* The space between rows does double duty: a drop zone to break a photo out
     into its own row, and — on hover — the place to insert a paragraph. */
  .gap {
    position: relative;
    height: 28px;
    margin: 4px 0;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background var(--duration-instant) var(--ease-soft);
  }
  .gap.end {
    height: 36px;
  }
  .gap.over {
    background: color-mix(in srgb, var(--color-accent) 30%, transparent);
  }
  .gap-add {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 10px;
    border-radius: 999px;
    background: var(--color-surface-high);
    border: 1px solid var(--color-border);
    color: var(--color-muted);
    font-family: var(--font-header, sans-serif);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    opacity: 0;
    transition: opacity var(--duration-instant) var(--ease-soft);
  }
  .gap:hover .gap-add {
    opacity: 1;
  }
  .gap-add:hover {
    color: var(--color-foreground);
    border-color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
  }
  /* While dragging a photo the gap is a drop target — the button must not
     swallow the drop. Gated on the whole drag (.composer.dragging), NOT on
     .gap.over: keying it to .over meant the button was still hittable on
     the frame the pointer first crossed into the gap, so the hit-tested
     node flipped button -> gap -> button as .over toggled, and WebKit kept
     restarting the drag target instead of settling. Stable hit area for the
     entire drag is the point. */
  .composer.dragging .gap-add {
    opacity: 0;
    pointer-events: none;
  }
  /* During a drag the ONLY things allowed to take hits are the drop targets
     themselves (.photo-cell and .gap). Everything interactive inside a cell
     is neutralised for the duration:
       - .caption is an <input>, which in WebKit is a NATIVE drop target for
         text/plain — and text/plain is exactly what onDragStart puts on the
         dataTransfer. Dropping on it types the block id into the caption
         instead of reordering. This one is a real data-corruption bug, not
         just a missed drop.
       - the overlay buttons would otherwise change the hit-tested node as
         the pointer crosses them, restarting the drop target mid-drag. */
  .composer.dragging .caption,
  .composer.dragging .cell-remove,
  .composer.dragging .cell-break,
  .composer.dragging .prose,
  .composer.dragging .add-prose,
  .composer.dragging .row-tools {
    pointer-events: none;
  }

  .add-prose {
    all: unset;
    align-self: center;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    padding: 6px 14px;
    border-radius: 999px;
    border: 1px solid var(--color-border);
    color: var(--color-muted);
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .add-prose:hover {
    color: var(--color-foreground);
    border-color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
  }

  .cell-break {
    position: absolute;
    top: 6px;
    left: 6px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.65);
    color: var(--color-foreground);
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s var(--ease-standard), background 0.15s var(--ease-standard);
    z-index: 2;
  }
  .photo-cell:hover .cell-break {
    opacity: 1;
  }
  .cell-break:hover {
    background: var(--color-accent);
    color: #fff;
  }

  .row-tools {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 0.6rem;
  }
  .split-all-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-family: var(--font-monospace, monospace);
    font-size: 0.7rem;
    padding: 0.25rem 0.6rem;
    background: var(--color-surface-high);
    border: var(--border);
    border-radius: var(--radius);
    color: var(--color-foreground);
    cursor: pointer;
  }
  .split-all-btn:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
</style>
