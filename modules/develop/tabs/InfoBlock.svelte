<script>
  import { invoke } from "@tauri-apps/api/core";
  import { Icon } from "@modules/core";

  let {
    picked,
    exifLine,
    exif,
    renderMs,
    status,
    rating,
    caption = $bindable(),
    /** @type {string[]} */
    tags = $bindable([]),
    photoPath,
    // What happens after `caption`/`tags` change differs by host (docked:
    // save directly; detached: relay over IPC) — same pattern as
    // DevelopPanel's other action props, so this stays injected rather than
    // hardcoded here.
    onCaptionEdited = () => {},
    onTagsEdited = () => {},
  } = $props();

  let tagInput = $state("");
  let generatingTags = $state(false);
  let tagsError = $state("");
  let captionEl = $state(/** @type {HTMLTextAreaElement | null} */ (null));

  /**
   * Wraps the current textarea selection in markdown markers (or, with
   * nothing selected, drops the caret between them) — the same
   * insert-around-selection behaviour every markdown editor's B/I toolbar
   * buttons use. Re-selects the wrapped text afterward so hitting Bold
   * again toggles it back off intuitively.
   * @param {string} before @param {string} after
   */
  function wrapSelection(before, after) {
    const el = captionEl;
    if (!el) return;
    const start = el.selectionStart ?? caption.length;
    const end = el.selectionEnd ?? caption.length;
    const selected = caption.slice(start, end);
    caption = caption.slice(0, start) + before + selected + after + caption.slice(end);
    onCaptionEdited();
    queueMicrotask(() => {
      el.focus();
      const caretStart = start + before.length;
      el.setSelectionRange(caretStart, caretStart + selected.length);
    });
  }

  function copyPath() {
    if (photoPath) navigator.clipboard?.writeText(photoPath);
  }

  /** @param {string} raw */
  function addTag(raw) {
    const tag = raw.trim().toLowerCase();
    if (!tag || tags.includes(tag)) return;
    tags = [...tags, tag];
    onTagsEdited();
  }

  function commitTagInput() {
    addTag(tagInput);
    tagInput = "";
  }

  /** @param {KeyboardEvent} e */
  function onTagInputKeydown(e) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      commitTagInput();
    } else if (e.key === "Backspace" && !tagInput && tags.length) {
      tags = tags.slice(0, -1);
      onTagsEdited();
    }
  }

  /** @param {string} tag */
  function removeTag(tag) {
    tags = tags.filter((t) => t !== tag);
    onTagsEdited();
  }

  async function generateTags() {
    if (!photoPath || generatingTags) return;
    generatingTags = true;
    tagsError = "";
    try {
      const suggested = await invoke("generate_tags", { path: photoPath });
      let changed = false;
      for (const raw of suggested) {
        const tag = String(raw).trim().toLowerCase();
        if (tag && !tags.includes(tag)) {
          tags = [...tags, tag];
          changed = true;
        }
      }
      if (changed) onTagsEdited();
    } catch (error) {
      tagsError = String(error);
    } finally {
      generatingTags = false;
    }
  }

  /** @param {string} path */
  function parentDir(path) {
    if (!path) return "";
    const parts = path.split("/");
    parts.pop();
    return parts.join("/");
  }

  function revealInFinder() {
    if (photoPath) {
      invoke("open_path", { path: parentDir(photoPath) }).catch(() => {});
    }
  }
</script>

<div class="info-block">
  <span class="din info-name">{picked ? picked.replace(/\.[^.]+$/, "") : "—"}</span>
  {#if exifLine}<span class="mono">{exifLine}</span>{/if}
  {#if exif?.captured_at}<span class="mono">{exif.captured_at}</span>{/if}
  {#if exif?.make || exif?.model}
    <span class="mono dim">{[exif.make, exif.model].filter(Boolean).join(" ")}</span>
  {/if}
  {#if exif?.width && exif?.height}
    <span class="mono dim">{exif.width} × {exif.height} px · {photoPath?.startsWith("apple-photos://") ? "Apple Photos" : "RAW + preview"}</span>
  {/if}
  {#if renderMs}<span class="mono dim">Rendered in {renderMs} ms</span>{/if}
  {#if status}<span class="mono">{status}</span>{/if}
  {#if rating > 0}
    <span class="info-stars" aria-label="{rating} stars">{"★".repeat(rating)}</span>
  {/if}
  <div class="caption-block">
    <div class="caption-toolbar">
      <span class="din">Caption</span>
      <div class="toolbar-actions">
        <button class="ghost toolbar-btn" onclick={() => wrapSelection("**", "**")} title="Bold">
          <Icon name="text-b" size="11px" />
        </button>
        <button class="ghost toolbar-btn" onclick={() => wrapSelection("*", "*")} title="Italic">
          <Icon name="text-italic" size="11px" />
        </button>
      </div>
    </div>
    <textarea
      bind:this={captionEl}
      class="caption"
      rows="6"
      placeholder="Write a caption… **bold**, *italic* — shown as a caption callout when shared to the daily log."
      bind:value={caption}
      oninput={() => onCaptionEdited()}
    ></textarea>
  </div>
  <div class="tags-block">
    <div class="tags-header">
      <span class="din">Tags</span>
      <button
        class="ghost ai-btn"
        onclick={generateTags}
        disabled={!photoPath || generatingTags}
        title="Suggest tags with AI"
      >
        <Icon name="sparkle" size="10px" />
        {generatingTags ? "Generating…" : "Generate"}
      </button>
    </div>
    <div class="tag-list">
      {#each tags as tag (tag)}
        <span class="tag-chip">
          {tag}
          <button class="tag-remove" onclick={() => removeTag(tag)} title="Remove tag">
            <Icon name="x" size="8px" />
          </button>
        </span>
      {/each}
      <input
        class="tag-input"
        type="text"
        placeholder={tags.length ? "" : "Add tag…"}
        bind:value={tagInput}
        onkeydown={onTagInputKeydown}
        onblur={commitTagInput}
      />
    </div>
    {#if tagsError}<span class="mono tags-error">{tagsError}</span>{/if}
  </div>
  {#if photoPath}
    <div class="path-row">
      <span class="mono path">{photoPath.startsWith("apple-photos://") ? "Apple Photos (read-only original)" : parentDir(photoPath)}</span>
      <button class="ghost" onclick={copyPath} title="Copy path">
        <Icon name="copy" size="10px" />
      </button>
      <button class="ghost" onclick={revealInFinder} title="Reveal in Finder" disabled={photoPath.startsWith("apple-photos://")}>
        <Icon name="folder-open" size="10px" />
      </button>
    </div>
  {/if}
</div>

<style>
  .info-block {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 3);
  }
  .mono.dim {
    color: var(--color-muted);
  }

  .caption-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }
  .caption-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .toolbar-actions {
    display: flex;
    gap: var(--space-d8);
  }
  .toolbar-btn {
    padding: var(--space-d5);
  }

  /* The editorial surface of this panel — everything else here is metadata
     to glance at, this is the thing you actually write in. Given real
     height and a serif/reading font (not the mono/din used everywhere
     else) so it reads as prose, not a form field. */
  .caption {
    padding: calc(var(--space-d4) * 3) calc(var(--space-d4) * 3);
    resize: vertical;
  }

  .tags-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }
  .tags-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .ai-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d4);
  }
  .ai-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .tag-list {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-d4);
    min-height: calc(var(--font-text-size, 13px) * var(--line-height, 1.4) + 8px);
    padding: var(--space-d4) var(--space-d3);
    background: var(--color-surface);
    border: var(--stroke-width) solid var(--color-border);
    border-radius: var(--radius);
  }
  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d5);
    background: var(--color-surface-light-2);
    border-radius: var(--radius-lg);
    padding: var(--space-d8) var(--space-d4) var(--space-d8) var(--space-d2);
  }
  .tag-remove {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    padding: var(--space-d8);
  }
  .tag-input {
    flex: 1;
    min-width: 60px;
  }
  .tags-error {
    color: var(--color-red);
  }

  .path-row {
    display: flex;
    align-items: center;
    gap: var(--space-d4);
  }
  .path {
    color: var(--color-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }

  .ghost {
    cursor: pointer;
    display: inline-flex;
  }
</style>
