<script>
  // The Story composer — a WYSIWYG port of the Swift compose surface
  // (CullView.swift `StoryPhotoRow`/`StoryPhotoCell`): the note is shown as it
  // will read on the Garden — photos in justified rows, captions in place,
  // prose between — and edited directly. No markdown editor; the block model
  // (lib/story.js) keeps the file a plain hand-editable note underneath.
  /** @import { Block } from "$lib/story.js" */
  /** @typedef {ReturnType<typeof storyRows>[number]} StoryRow */
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
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

  // A photo's caption IS the photo's caption, whichever surface you write
  // it from — this story note's own "> [!caption]" text (setText, below)
  // is one place it's stored, but Develop mode's caption box reads/writes
  // the RAW's own sidecar (description) independently, so a caption typed
  // here never showed up there. Mirroring every photo-caption edit into
  // the sidecar too (paragraph blocks have no stem — they're not a photo's
  // caption, so they're untouched) keeps the two in sync going forward.
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
    if (id === beforeId) return;
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
   * Move an entire row (prose or photo row) before or after another row.
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
    const isProse = blocks.find((b) => b.id === id)?.isPhoto === false;

    if (mode === "before" || (isProse && mode === "group")) {
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
      } else if (e.dataTransfer.setDragImage) {
        const rowEl = target.closest(".prose-row");
        if (rowEl instanceof HTMLElement) {
          e.dataTransfer.setDragImage(rowEl, 40, 20);
        }
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

  /**
   * Auto-expand textarea to fit its text without scrollbars or resize handles.
   * @param {HTMLTextAreaElement} node
   */
  function autoExpand(node) {
    const update = () => {
      node.style.height = "auto";
      node.style.height = `${node.scrollHeight}px`;
    };
    node.addEventListener("input", update);
    requestAnimationFrame(update);
    return {
      update,
      destroy() {
        node.removeEventListener("input", update);
      },
    };
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
      <div class="empty">
        <Icon name="stack-simple" size="32px" />
        <p>Glisse ou clique une photo de la pellicule pour commencer l'histoire.</p>
      </div>
    {:else if blocks.filter((b) => b.isPhoto).length > 1}
      <div class="row-tools">
        {#if rows.some((r) => r.type === "photos" && r.blocks.length > 1)}
          <button class="split-all-btn" onclick={splitAll} title="Place each photo on its own row">
            <Icon name="rows" size="12px" /><span>Split into individual rows</span>
          </button>
        {/if}
        <button class="split-all-btn" onclick={sortByDate} title="Reorder photos chronologically">
          <Icon name="arrows-down-up" size="12px" /><span>Chronologie</span>
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
        <button class="gap-add" onclick={() => addProseAt(r)} title="Insert a paragraph here">
          <Icon name="plus" size="10px" /><span>Paragraphe</span>
        </button>
      </div>

      {#if row.type === "prose"}
        <div
          class="prose-row"
          class:dragging={dragId === row.block.id}
          data-row-first={row.block.id}
        >
          <div class="prose-container">
            <button
              type="button"
              class="row-grip"
              draggable="true"
              ondragstart={(e) => onDragStart(row.block.id, e)}
              ondragend={onDragEnd}
              title="Drag to move this text"
              aria-label="Move this paragraph"
            >
              <Icon name="dots-six-vertical" size="14px" />
            </button>

            <textarea
              use:autoExpand
              class="prose"
              rows="1"
              placeholder="Write a paragraph, a thought, or the story of a moment…"
              value={row.block.text}
              oninput={(e) => setText(row.block.id, e.currentTarget.value)}
            ></textarea>

            <div class="row-side-actions">
              {#if r > 0}
                <button
                  type="button"
                  class="row-action-btn"
                  onclick={() => moveRow(r, r - 1)}
                  title="Move this paragraph up"
                  aria-label="Move this paragraph up"
                >
                  <Icon name="caret-up" size="10px" />
                </button>
              {/if}
              {#if r < rows.length - 1}
                <button
                  type="button"
                  class="row-action-btn"
                  onclick={() => moveRow(r, r + 1)}
                  title="Move this paragraph down"
                  aria-label="Move this paragraph down"
                >
                  <Icon name="caret-down" size="10px" />
                </button>
              {/if}
              <button
                type="button"
                class="row-action-btn row-remove"
                onclick={() => remove(row.block.id)}
                title="Remove this text"
                aria-label="Delete this paragraph"
              >
                <Icon name="x" size="10px" />
              </button>
            </div>
          </div>
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
                  <div class="missing" title="Photo missing from the folder">{cell.block.stem}</div>
                {/if}
                {#if row.blocks.length > 1}
                  <button class="cell-break" onclick={() => breakOut(cell.block.id)} title="Split onto its own row">
                    <Icon name="rows" size="11px" />
                  </button>
                {/if}
                <button class="cell-remove" onclick={() => remove(cell.block.id)} title="Remove from story">
                  <Icon name="x" size="10px" />
                </button>
              </div>
              <input
                class="caption"
                placeholder="Add a caption…"
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
        <Icon name="plus" size="12px" />
        <span>Add a paragraph</span>
      </button>
    {/if}
  </div>

  <!-- The film roll sits UNDER the story: the composition is what you're
       reading, the roll is the tray you pick from. Click a frame to append it. -->
  {#if frames.length}
    <div class="roll">
      <div class="roll-header">
        <Icon name="stack-simple" size="13px" />
        <span class="roll-title">PELLICULE · {frames.length}</span>
      </div>
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
              <span class="roll-check"><Icon name="check" size="9px" /></span>
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
    background: var(--theme-bg, transparent);
    transition: background 0.3s var(--ease-standard);
  }

  /* Film roll — docked tray at bottom */
  /* The same floating pane as the sidebar (Sidebar.svelte's nav): inset
     from the window, concentric corner, raised — not a bar welded to the
     bottom edge. overflow: hidden keeps the scrolling strip inside the
     rounded corners. */
  .roll {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 1rem;
    margin: 0 var(--window-inset) var(--window-inset);
    padding: 0.65rem 1.25rem;
    background: var(--color-surface-high);
    border-radius: var(--pane-radius);
    box-shadow: var(--shadow-raised), var(--shadow-lift);
    overflow: hidden;
    z-index: 10;
  }
  .roll-header {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    color: var(--color-muted);
  }
  .roll-title {
    font-family: var(--font-header, sans-serif);
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.1em;
    user-select: none;
    white-space: nowrap;
  }
  .roll-strip {
    display: flex;
    gap: 0.5rem;
    overflow-x: auto;
    padding: 6px;
    scrollbar-width: thin;
  }
  .roll-cell {
    all: unset;
    position: relative;
    flex-shrink: 0;
    width: 60px;
    height: 60px;
    border:none;
    border-radius: var(--radius);
    overflow: visible;
    cursor: pointer;
    box-shadow: var(--shadow);
    transition: all var(--transition-fast);
  }
  .roll-cell:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow-hover);
  }
  .roll-cell.in-story {
    box-shadow: 0 0 0 2px var(--theme-accent, var(--color-accent));
  }
  .roll-cell img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    pointer-events: none;
    -webkit-user-drag: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .roll-check {
    position: absolute;
    right: 3px;
    bottom: 3px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: var(--color-accent);
    color: #fff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-shadow: var(--shadow);
  }

  /* The composition surface — the story as it will read. */
  .surface {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    padding: 24px max(24px, 5vw) 120px;
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
    gap: 1rem;
    color: var(--color-muted);
    font-family: var(--font-text, sans-serif);
    font-size: 0.95rem;
    text-align: center;
    padding: 5rem 1rem;
    opacity: 0.6;
  }
  .empty p {
    margin: 0;
    max-width: 24rem;
    line-height: 1.5;
  }

  .row-tools {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    margin-bottom: 1.25rem;
  }
  .split-all-btn {
    all: unset;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-header, sans-serif);
    font-size: 11px;
    letter-spacing: 0.04em;
    padding: 5px 10px;
    background: var(--color-surface-low);
    border: 1px solid var(--color-border);
    border-radius: var(--theme-radius, var(--radius-sm));
    color: var(--color-muted);
    cursor: pointer;
    transition: all 0.15s var(--ease-soft);
  }
  .split-all-btn:hover {
    background: var(--color-surface-high);
    border-color: var(--theme-accent, color-mix(in srgb, var(--color-foreground) 30%, transparent));
    color: var(--theme-accent, var(--color-foreground));
  }

  .photo-row {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    margin: 8px 0;
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
    border-radius: var(--theme-radius, var(--radius-sm));
    overflow: hidden;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.45), 0 0 0 1px rgba(255, 255, 255, 0.08);
    transition: box-shadow 0.15s var(--ease-standard), transform 0.15s var(--ease-standard);
  }
  .photo-cell.drop-group .frame {
    box-shadow: 0 0 0 2px var(--theme-accent, var(--color-accent)), 0 6px 24px rgba(0, 0, 0, 0.5);
  }
  .photo-cell.drop-before::before {
    content: "";
    position: absolute;
    top: -6px;
    left: -4px;
    right: -4px;
    height: 4px;
    background: var(--theme-accent, var(--color-accent));
    border-radius: var(--theme-radius, var(--radius-sm));
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
    background: var(--theme-accent, var(--color-accent));
    border-radius: var(--theme-radius, var(--radius-sm));
    z-index: 10;
    pointer-events: none;
  }
  .frame img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    /* WebKit doesn't reliably clip an object-fit img to an ancestor's
       overflow:hidden + border-radius — its square corners can still peek
       past the rounded mask. Rounding the img itself too closes the gap. */
    border-radius: inherit;
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
  .cell-break {
    position: absolute;
    top: 8px;
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    background: rgba(0, 0, 0, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.18);
    color: #fff;
    opacity: 0;
    transition: opacity var(--duration-fast) var(--ease-soft), background var(--duration-fast) var(--ease-soft), transform var(--duration-fast) var(--ease-soft);
    z-index: 2;
  }
  .cell-remove {
    right: 8px;
  }
  .cell-break {
    left: 8px;
  }
  .frame:hover .cell-remove,
  .frame:hover .cell-break {
    opacity: 0.85;
  }
  .cell-remove:hover {
    opacity: 1;
    background: rgba(220, 38, 38, 0.85);
    transform: scale(1.08);
  }
  .cell-break:hover {
    opacity: 1;
    background: var(--theme-accent, var(--color-accent));
    transform: scale(1.08);
  }

  .caption {
    all: unset;
    display: block;
    box-sizing: border-box;
    width: 100%;
    text-align: center;
    font-family: var(--theme-font-text, var(--font-text, sans-serif));
    font-size: 0.82rem;
    letter-spacing: 0.02em;
    font-style: italic;
    color: color-mix(in srgb, var(--theme-text-color, var(--color-foreground)) 65%, transparent);
    padding: 6px 10px;
    border-bottom: 1px solid transparent;
    transition: color 0.15s var(--ease-soft), border-color 0.15s var(--ease-soft);
  }
  .caption::placeholder {
    color: color-mix(in srgb, var(--theme-text-color, var(--color-foreground)) 25%, transparent);
    font-style: italic;
  }
  .caption:focus {
    color: var(--theme-text-color, var(--color-foreground));
    border-bottom-color: var(--theme-accent, var(--color-accent));
  }

  .prose-row {
    position: relative;
    margin: 12px 0;
    display: flex;
    justify-content: center;
  }
  .prose-row.dragging {
    opacity: 0.35;
  }
  .prose-container {
    position: relative;
    width: 100%;
    max-width: 44rem;
    margin: 0 auto;
    display: flex;
    align-items: flex-start;
  }
  .row-grip {
    position: absolute;
    top: 14px;
    left: -32px;
    all: unset;
    box-sizing: border-box;
    cursor: grab;
    width: 24px;
    height: 24px;
    border-radius: var(--theme-radius, var(--radius-sm));
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-muted);
    background: var(--color-surface-low);
    border: 1px solid var(--color-border);
    opacity: 0;
    transition: opacity var(--duration-fast) var(--ease-soft), background var(--duration-fast) var(--ease-soft), color var(--duration-fast) var(--ease-soft);
    z-index: 3;
  }
  .prose-row:hover .row-grip {
    opacity: 0.75;
  }
  .row-grip:hover {
    opacity: 1;
    color: var(--color-foreground);
    background: var(--color-surface-high);
    border-color: color-mix(in srgb, var(--color-foreground) 30%, transparent);
  }
  .row-grip:active {
    cursor: grabbing;
  }

  .row-side-actions {
    position: absolute;
    top: 14px;
    right: -34px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    opacity: 0;
    transition: opacity var(--duration-fast) var(--ease-soft);
    z-index: 3;
  }
  .prose-row:hover .row-side-actions {
    opacity: 0.85;
  }
  .row-side-actions:hover {
    opacity: 1;
  }
  .row-action-btn {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--color-surface-low);
    border: 1px solid var(--color-border);
    color: var(--color-muted);
    transition: all var(--duration-fast) var(--ease-soft);
  }
  .row-action-btn:hover {
    background: var(--color-surface-high);
    color: var(--color-foreground);
    border-color: color-mix(in srgb, var(--color-foreground) 30%, transparent);
  }
  .row-action-btn.row-remove:hover {
    background: rgba(220, 38, 38, 0.85);
    border-color: transparent;
    color: #fff;
  }

  .prose {
    all: unset;
    display: block;
    box-sizing: border-box;
    width: 100%;
    margin: 0 auto;
    font-family: var(--theme-font-text, var(--font-text, Georgia, serif));
    font-size: 1.12rem;
    line-height: 1.75;
    color: var(--theme-text-color, var(--color-foreground));
    padding: 14px 20px;
    border-radius: var(--theme-radius, var(--radius-sm));
    border: 1px solid transparent;
    resize: none;
    overflow: hidden;
    text-align: left;
    background: transparent;
    transition: font-family 0.25s var(--ease-standard), color 0.3s var(--ease-standard), border-color var(--duration-fast) var(--ease-soft), background var(--duration-fast) var(--ease-soft);
  }
  .prose:hover {
    background: color-mix(in srgb, var(--theme-text-color, var(--color-foreground)) 3%, transparent);
  }
  .prose:focus {
    background: color-mix(in srgb, var(--theme-text-color, var(--color-foreground)) 5%, transparent);
    border-color: color-mix(in srgb, var(--theme-accent, var(--color-accent)) 40%, transparent);
  }
  .prose::placeholder {
    color: color-mix(in srgb, var(--theme-text-color, var(--color-foreground)) 25%, transparent);
    font-style: italic;
  }

  /* The space between rows: subtle seam that appears cleanly on hover */
  .gap {
    position: relative;
    height: 32px;
    margin: 4px 0;
    flex-shrink: 0;
    border-radius: var(--theme-radius, var(--radius-sm));
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background var(--duration-instant) var(--ease-soft);
  }
  .gap::before {
    content: "";
    position: absolute;
    left: 20%;
    right: 20%;
    height: 1px;
    background: color-mix(in srgb, var(--color-border) 40%, transparent);
    opacity: 0;
    transition: opacity 0.15s var(--ease-soft);
    pointer-events: none;
  }
  .gap:hover::before {
    opacity: 1;
  }
  .gap.end {
    height: 40px;
  }
  .gap.over {
    background: color-mix(in srgb, var(--theme-accent, var(--color-accent)) 25%, transparent);
  }
  .gap-add {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 12px;
    border-radius: 999px;
    background: var(--color-surface-high);
    color: var(--color-muted);
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    opacity: 0;
    transform: scale(0.96);
    transition: opacity var(--duration-fast) var(--ease-soft), transform var(--duration-fast) var(--ease-soft), color var(--duration-fast) var(--ease-soft), border-color var(--duration-fast) var(--ease-soft);
    box-shadow: var(--shadow);
    z-index: 2;
  }
  .gap:hover .gap-add {
    opacity: 1;
    transform: scale(1);
  }
  .gap-add:hover {
    color: var(--color-foreground);
    border-color: var(--theme-accent, var(--color-accent));
    background: var(--color-surface-higher, var(--color-surface-high));
  }

  .composer.dragging .gap-add {
    opacity: 0;
    pointer-events: none;
  }
  .composer.dragging .caption,
  .composer.dragging .cell-remove,
  .composer.dragging .cell-break,
  .composer.dragging .prose,
  .composer.dragging .row-grip,
  .composer.dragging .row-side-actions,
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
    gap: 8px;
    margin-top: 24px;
    padding: 10px 22px;
    border-radius: 999px;
    background: var(--color-surface-low);
    border: 1px dashed var(--color-border);
    color: var(--color-muted);
    font-family: var(--font-header, sans-serif);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    transition: all 0.15s var(--ease-soft);
  }
  .add-prose:hover {
    color: var(--color-foreground);
    border-style: solid;
    border-color: var(--theme-accent, var(--color-accent));
    background: var(--color-surface-high);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.3);
  }
</style>
