<script>
  // Pinned + Recent story-note lists — a port of Swift `StoryNotesSidebar`
  // (CullView.swift:2150-2280). Pinned rows are drag-reorderable; the
  // reorder callback rewrites `pinned-at` ordering. Recent is read-only,
  // sorted by mtime (top 12, sliced in the parent).

  import Icon from "$lib/components/Icon.svelte";

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
    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)} min ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)} h ago`;
    if (diff < 172800) return "yesterday";
    if (diff < 604800) return `${Math.floor(diff / 86400)} d ago`;
    return new Date(mtime * 1000).toLocaleDateString("en-CA", { month: "short", day: "numeric" });
  }
</script>

<div class="notes-list">
  <section class="group">
    <div class="group-header">
      <span class="group-title">Pinned</span>
      {#if pinned.length > 0}
        <span class="badge">{pinned.length}</span>
      {/if}
    </div>
    {#if pinned.length === 0}
      <p class="empty">No pinned stories</p>
    {:else}
      <div class="notes-items" role="list">
        {#each pinned as note, i (note.notePath)}
          <div
            class="note-row pinned"
            class:current={note.folderPath === curDir}
            class:dragging={dragIndex === i}
            draggable="true"
            role="listitem"
            ondragstart={(e) => onDragStart(e, i)}
            ondragover={(e) => onDragOver(e, i)}
            ondrop={(e) => onDrop(e, i)}
            ondragend={onDragEnd}
          >
            <span class="grip" title="Drag to reorder" aria-hidden="true">
              <Icon name="dots-six-vertical" size="12px" />
            </span>
            <button type="button" class="note-open" aria-label={`Open ${note.folderName}`} onclick={() => onOpen(note.folderPath)}>
              <div class="note-meta">
                <span class="folder-name">{note.folderName}</span>
                <span class="thumbs">{note.thumbStems.length} photo{note.thumbStems.length > 1 ? "s" : ""}</span>
              </div>
            </button>
            <button
              type="button"
              class="action-btn unpin"
              aria-label={`Unpin ${note.folderName}`}
              title="Unpin"
              onclick={() => onUnpin(note.notePath)}
            >
              <Icon name="x" size="10px" />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <section class="group">
    <div class="group-header">
      <span class="group-title">Recent</span>
      {#if recent.length > 0}
        <span class="badge">{recent.length}</span>
      {/if}
    </div>
    {#if recent.length === 0}
      <p class="empty">No recent stories</p>
    {:else}
      <div class="notes-items" role="list">
        {#each recent as note (note.notePath)}
          <div
            class="note-row"
            class:current={note.folderPath === curDir}
            role="listitem"
          >
            <button type="button" class="note-open" aria-label={`Open ${note.folderName}`} onclick={() => onOpen(note.folderPath)}>
              <div class="note-meta">
                <span class="folder-name">{note.folderName}</span>
                <span class="mtime">{relMtime(note.mtime)}</span>
              </div>
            </button>
            {#if !note.pinned}
              <button
                class="action-btn pin"
                type="button"
                aria-label={`Pin ${note.folderName}`}
                title="Pin"
                onclick={() => onPin(note.notePath)}
              >
                <Icon name="plus" size="10px" />
              </button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .notes-list {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .group-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px;
  }
  .group-title {
    font-family: var(--font-header, sans-serif);
    font-size: 0.65rem;
    letter-spacing: 0.12em;
    color: var(--color-muted);
    text-transform: uppercase;
    font-weight: 600;
  }
  .badge {
    font-family: var(--font-monospace, monospace);
    font-size: 0.62rem;
    color: var(--color-muted);
    background: var(--color-surface-high);
    padding: 1px 5px;
    border-radius: 999px;
    border: 1px solid var(--color-border);
  }
  .notes-items {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
  }
  .note-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: var(--radius);
    background: var(--color-surface-low);
    border: 1px solid var(--color-border);
    transition: all 0.15s var(--ease-soft);
  }
  .note-row:hover {
    background: var(--color-surface-high);
    border-color: color-mix(in srgb, var(--color-foreground) 25%, transparent);
  }
  .note-row.current {
    background: var(--color-surface-high);
    border-color: var(--color-accent);
    box-shadow: 0 0 0 1px var(--color-accent);
  }
  .note-row.dragging {
    opacity: 0.35;
  }
  .note-open {
    all: unset;
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
    cursor: pointer;
  }
  .note-open:focus-visible,
  .action-btn:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
  }
  .note-row.pinned .grip {
    opacity: 0.25;
    cursor: grab;
    font-size: 11px;
    user-select: none;
    transition: opacity 0.15s var(--ease-soft);
  }
  .note-row.pinned:hover .grip {
    opacity: 0.7;
  }
  .note-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  .folder-name {
    font-family: var(--font-header, sans-serif);
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--color-foreground);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .thumbs,
  .mtime {
    font-family: var(--font-monospace, monospace);
    font-size: 0.65rem;
    color: var(--color-muted);
  }
  .action-btn {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-muted);
    opacity: 0;
    transition: all 0.15s var(--ease-soft);
  }
  .note-row:hover .action-btn {
    opacity: 0.7;
  }
  .action-btn:hover {
    opacity: 1;
    background: var(--color-surface-higher, var(--color-surface-high));
    color: var(--color-foreground);
  }
  .action-btn.unpin:hover {
    color: #ef4444;
  }
  .action-btn.pin:hover {
    color: var(--color-accent);
  }
  .empty {
    font-family: var(--font-text, sans-serif);
    font-size: 0.75rem;
    color: var(--color-muted);
    opacity: 0.5;
    margin: 4px 6px;
    font-style: italic;
  }
</style>
