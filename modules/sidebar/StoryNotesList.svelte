<script>
  // ÉPINGLÉES (pinned) + RÉCENTES (recent) story-note lists — a port of
  // Swift `StoryNotesSidebar` (CullView.swift:2150-2280). Pinned rows are
  // drag-reorderable; the reorder callback rewrites `pinned-at` ordering.
  // RÉCENTES is read-only, sorted by mtime (top 12, sliced in the parent).

  let {
    pinned = [],
    recent = [],
    curDir = null,
    onOpen = () => {},      // (folderPath) => void
    onUnpin = () => {},     // (notePath) => void
    onPin = () => {},       // (notePath) => void
    onReorder = () => {},   // (fromIndex, toIndex) => void
  } = $props();

  /** @type {number | null} */
  let dragIndex = $state(null);

  /** @param {DragEvent} e @param {number} i */
  function onDragStart(e, i) {
    dragIndex = i;
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }
  /** @param {DragEvent} e @param {number} i */
  function onDragOver(e, i) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  }
  /** @param {DragEvent} e @param {number} i */
  function onDrop(e, i) {
    e.preventDefault();
    if (dragIndex === null || dragIndex === i) return;
    onReorder(dragIndex, i);
    dragIndex = null;
  }
  function onDragEnd() { dragIndex = null; }

  /** @param {number} mtime */
  function relMtime(mtime) {
    const diff = Date.now() / 1000 - mtime;
    if (diff < 3600) return "just now";
    if (diff < 86400) return `${Math.floor(diff / 3600)} h ago`;
    if (diff < 604800) return `${Math.floor(diff / 86400)} d ago`;
    return new Date(mtime * 1000).toLocaleDateString("en-CA");
  }
</script>

<div class="notes-list">
  <section class="group">
    <h3 class="group-title">Pinned</h3>
    {#if pinned.length === 0}
      <p class="empty">No pinned stories</p>
    {:else}
      <ul>
        {#each pinned as note, i (note.notePath)}
          <li
            class="note-row pinned"
            class:current={note.folderPath === curDir}
            class:dragging={dragIndex === i}
            draggable="true"
            ondragstart={(e) => onDragStart(e, i)}
            ondragover={(e) => onDragOver(e, i)}
            ondrop={(e) => onDrop(e, i)}
            ondragend={onDragEnd}
          >
            <span class="grip" title="Drag to reorder" aria-hidden="true">⠿</span>
            <button type="button" class="note-open" aria-label={`Open ${note.folderName}`} onclick={() => onOpen(note.folderPath)}>
              <span class="folder-name">{note.folderName}</span>
              <span class="thumbs">{note.thumbStems.length} photos</span>
            </button>
            <button
              type="button"
              class="unpin"
              aria-label={`Unpin ${note.folderName}`}
              title="Unpin"
              onclick={() => onUnpin(note.notePath)}
            >×</button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="group">
    <h3 class="group-title">Recent</h3>
    {#if recent.length === 0}
      <p class="empty">No recent stories</p>
    {:else}
      <ul>
        {#each recent as note (note.notePath)}
          <li
            class="note-row"
            class:current={note.folderPath === curDir}
          >
            <button type="button" class="note-open" aria-label={`Open ${note.folderName}`} onclick={() => onOpen(note.folderPath)}>
              <span class="folder-name">{note.folderName}</span>
              <span class="mtime">{relMtime(note.mtime)}</span>
            </button>
            {#if !note.pinned}
              <button
                class="pin"
                type="button"
                aria-label={`Pin ${note.folderName}`}
                title="Pin"
                onclick={() => onPin(note.notePath)}
              >＋</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .notes-list { display: flex; flex-direction: column; gap: 14px; }
  .group-title {
    font-size: 10px; letter-spacing: 0.08em; opacity: 0.5; margin: 0 0 6px;
    text-transform: uppercase; font-weight: 500;
  }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  .note-row {
    display: flex; align-items: center; gap: 8px; padding: 5px 8px; border-radius: var(--radius-sm);
    cursor: pointer; font-size: 12px; border: 1px solid transparent;
  }
  .note-row:hover { background: var(--color-surface-high); }
  .note-row.current { border-color: var(--border, rgba(255,255,255,0.15)); }
  .note-row.dragging { opacity: 0.4; }
  .note-open {
    all: unset; display: flex; align-items: center; gap: 8px; flex: 1;
    min-width: 0; cursor: pointer;
  }
  .note-open:focus-visible, .pin:focus-visible, .unpin:focus-visible {
    outline: 2px solid var(--color-accent); outline-offset: 2px;
  }
  .note-row.pinned .grip { opacity: 0.3; cursor: grab; font-size: 11px; }
  .note-row.pinned:hover .grip { opacity: 0.7; }
  .folder-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .thumbs, .mtime { font-size: 10px; opacity: 0.5; flex-shrink: 0; }
  .unpin, .pin {
    background: none; border: none; color: inherit; cursor: pointer; opacity: 0.3;
    font-size: 13px; line-height: 1; padding: 0 2px; flex-shrink: 0;
  }
  .unpin:hover, .pin:hover { opacity: 1; }
  .empty { font-size: 11px; opacity: 0.35; margin: 0; font-style: italic; }
</style>
