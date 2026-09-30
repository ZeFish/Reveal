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
      <p class="empty fine">No pinned stories</p>
    {:else}
      <div class="notes-items list" role="list">
        {#each pinned as note, i (note.notePath)}
          <div
            class="item note-row pinned"
            aria-current={note.folderPath === curDir ? "true" : undefined}
            class:is-dragging={dragIndex === i}
            data-reveal-host
            draggable="true"
            role="listitem"
            ondragstart={(e) => onDragStart(e, i)}
            ondragover={(e) => onDragOver(e, i)}
            ondrop={(e) => onDrop(e, i)}
            ondragend={onDragEnd}
          >
            <span class="grip" data-reveal title="Drag to reorder" aria-hidden="true">
              <Icon name="dots-six-vertical" size="12px" />
            </span>
            <button type="button" class="note-open ghost" aria-label={`Open ${note.folderName}`} onclick={() => onOpen(note.folderPath)}>
              <div class="note-meta">
                <span class="folder-name">{note.folderName}</span>
                <span class="thumbs">{note.thumbStems.length} photo{note.thumbStems.length > 1 ? "s" : ""}</span>
              </div>
            </button>
            <button
              type="button"
              class="action-btn ghost icon small unpin" data-reveal
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
      <p class="empty fine">No recent stories</p>
    {:else}
      <div class="notes-items list" role="list">
        {#each recent as note (note.notePath)}
          <div
            class="item note-row"
            aria-current={note.folderPath === curDir ? "true" : undefined}
            data-reveal-host
            role="listitem"
          >
            <button type="button" class="note-open ghost" aria-label={`Open ${note.folderName}`} onclick={() => onOpen(note.folderPath)}>
              <div class="note-meta">
                <span class="folder-name">{note.folderName}</span>
                <span class="mtime">{relMtime(note.mtime)}</span>
              </div>
            </button>
            {#if !note.pinned}
              <button
                class="action-btn ghost icon small pin" data-reveal
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
    gap: var(--space);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }
  .group-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--space-d4);
  }
  .notes-items {
    gap: var(--space-d4);
    margin: 0;
    padding: 0;
  }
  .note-row {
    position: relative;
  }
  .note-open {
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
  }
  .grip {
    cursor: grab;
    user-select: none;
  }
  .note-meta {
    display: flex;
    flex-direction: column;
    gap: var(--space-d8);
    min-width: 0;
    flex: 1;
  }
  .folder-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    margin: var(--space-d4) var(--space-d3);
  }
</style>
