<script>
  import { invoke } from "@tauri-apps/api/core";
  import { Icon } from "@modules/core";
  import CollapsibleGroup from "@modules/develop/engines/controls/CollapsibleGroup.svelte";
  import { explainAiError } from "./aiError.js";

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
      tagsError = explainAiError(error);
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

<div class="info-tab">
  <div class="info-scroll">
    <CollapsibleGroup label="Photo">
      <div class="facts">
        <div class="fact"><span class="k">Name</span><span class="v strong">{picked ? picked.replace(/\.[^.]+$/, "") : "—"}</span></div>
        {#if exifLine}
          <div class="fact"><span class="k">Exposure</span><span class="v">{exifLine}</span></div>
        {/if}
        {#if exif?.captured_at}
          <div class="fact"><span class="k">Shot</span><span class="v">{exif.captured_at}</span></div>
        {/if}
        {#if exif?.make || exif?.model}
          <div class="fact"><span class="k">Camera</span><span class="v">{[exif.make, exif.model].filter(Boolean).join(" ")}</span></div>
        {/if}
        {#if exif?.width && exif?.height}
          <div class="fact"><span class="k">Size</span><span class="v">{exif.width} × {exif.height} px</span></div>
          <div class="fact"><span class="k">Source</span><span class="v">{photoPath?.startsWith("apple-photos://") ? "Apple Photos" : "RAW + preview"}</span></div>
        {/if}
        {#if renderMs}
          <div class="fact"><span class="k">Render</span><span class="v">{renderMs} ms</span></div>
        {/if}
        {#if status}
          <div class="fact"><span class="k">Status</span><span class="v">{status}</span></div>
        {/if}
      </div>
    </CollapsibleGroup>

    <CollapsibleGroup label="Caption">
      <div class="block">
        <div class="toolbar">
          <button class="ghost toolbar-btn" onclick={() => wrapSelection("**", "**")} title="Bold">
            <Icon name="text-b" size="var(--icon-md)" />
          </button>
          <button class="ghost toolbar-btn" onclick={() => wrapSelection("*", "*")} title="Italic">
            <Icon name="text-italic" size="var(--icon-md)" />
          </button>
        </div>
        <!-- In a block of its own: WebKit does not count a <textarea> that is itself a flex item
             in its container's height, and draws it at the wrong place (over the facts above). -->
        <div class="caption-field">
        <textarea
          bind:this={captionEl}
          class="caption"
          rows="6"
          placeholder="Write a caption… **bold**, *italic* — shown as a caption callout when shared to the daily log."
          bind:value={caption}
          oninput={() => onCaptionEdited()}
        ></textarea>
        </div>
      </div>
    </CollapsibleGroup>

    <CollapsibleGroup label="Tags">
      <div class="block">
        <div class="tag-list">
          {#each tags as tag (tag)}
            <span class="tag-chip">
              {tag}
              <button class="tag-remove" onclick={() => removeTag(tag)} title="Remove tag" aria-label={`Remove tag ${tag}`}>
                <Icon name="x" size="var(--icon-sm)" />
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
        <button class="outline ai-btn" onclick={generateTags} disabled={!photoPath || generatingTags} title="Suggest tags with AI">
          <Icon name="sparkle" size="var(--icon-md)" />
          <span>{generatingTags ? "Generating…" : "Suggest tags"}</span>
        </button>
        {#if tagsError}<span class="mono tags-error">{tagsError}</span>{/if}
      </div>
    </CollapsibleGroup>
  </div>

  <!-- Pinned to the bottom, like Reset / Export on the Dev tab. -->
  {#if photoPath}
    <div class="footer">
      <span class="path" title={photoPath}>{photoPath.startsWith("apple-photos://") ? "Apple Photos (read-only original)" : parentDir(photoPath)}</span>
      <button class="ghost icon-btn" onclick={copyPath} title="Copy path" aria-label="Copy path">
        <Icon name="copy" size="var(--icon-md)" />
      </button>
      <button
        class="ghost icon-btn"
        onclick={revealInFinder}
        title="Reveal in Finder"
        aria-label="Reveal in Finder"
        disabled={photoPath.startsWith("apple-photos://")}
      >
        <Icon name="folder-open" size="var(--icon-md)" />
      </button>
    </div>
  {/if}
</div>

<style>
  .info-tab {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-d2);
  }

  .info-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    /* No scrollbar: it sat on top of the values (same as the other tabs). */
    scrollbar-width: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }

  /* Facts: one row per fact, label then value. Plain flex rows rather than a grid: WebKit
     sized the grid for one-line values and let wrapped ones spill over the next group. */
  .facts {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
    padding-block: var(--space-d3);
  }
  .fact {
    display: flex;
    align-items: baseline;
    gap: var(--space-d2);
  }
  .fact .k {
    flex: 0 0 4.5em;
    color: var(--color-muted);
  }
  .fact .v {
    flex: 1;
    min-width: 0;
    overflow-wrap: break-word;
    font-family: var(--font-monospace);
  }
  .fact .v.strong {
    color: var(--color-foreground);
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
    padding-block: var(--space-d3);
  }

  .toolbar {
    display: flex;
    gap: var(--space-d4);
  }
  .toolbar-btn,
  .icon-btn {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-h);
    height: var(--control-h);
    padding: 0;
  }

  /* The editorial surface of this panel: everything else here is metadata to glance at, this
     is the thing you actually write in, so it gets real height. */
  .caption-field {
    display: block;
  }
  .caption {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-d3);
    resize: vertical;
  }

  .tag-list {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-d4);
    min-height: var(--control-h);
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
  /* The input lives inside the tag list's own frame: no second frame of its own (it drew a box
     within a box and crowded the button below). */
  .tag-input {
    flex: 1;
    min-width: 60px;
    height: calc(var(--control-h) - 8px);
    min-height: 0;
    padding: 0 var(--space-d4);
    background: transparent;
    box-shadow: none;
    outline: none;
  }
  .tag-list:focus-within {
    border-color: color-mix(in srgb, var(--color-accent) 55%, var(--color-border));
  }
  .ai-btn {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-d3);
    width: 100%;
    min-height: var(--control-h);
    padding-block: 0;
  }
  .ai-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .tags-error {
    color: var(--color-red);
    font-family: var(--font-monospace);
    font-size: 0.85em;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }

  .footer {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    padding: var(--space-d3) 0;
    border-top: var(--border);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-subtle);
    font-family: var(--font-monospace);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
