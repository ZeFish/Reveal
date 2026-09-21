<script>
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "$lib/components/Icon.svelte";

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
  <textarea
    class="caption"
    rows="2"
    placeholder="Caption…"
    bind:value={caption}
    oninput={() => onCaptionEdited()}
  ></textarea>
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
    gap: 10px;
    padding: 16px;
  }

  .din {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }

  .info-name {
    font-size: 13px;
    color: var(--color-foreground);
  }

  .mono {
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    color: color-mix(in srgb, var(--color-foreground) 90%, transparent);
  }
  .mono.dim {
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
  }

  .info-stars {
    font-size: 10px;
    letter-spacing: 2px;
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
  }

  .caption {
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 6px;
    resize: none;
    outline: none;
  }

  .tags-block {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .tags-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .ai-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: var(--font-header, sans-serif);
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .ai-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .tag-list {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    min-height: calc(var(--font-text-size) * var(--line-height) + 8px);
    padding: 4px 6px;
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
  }
  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-family: var(--font-monospace, monospace);
    font-size: 10px;
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    border-radius: 9999px;
    padding: 2px 4px 2px 8px;
  }
  .tag-remove {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    padding: 2px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .tag-remove:hover {
    color: var(--color-foreground);
  }
  .tag-input {
    all: unset;
    flex: 1;
    min-width: 60px;
    font-family: var(--font-monospace, monospace);
    font-size: 10px;
    color: var(--color-foreground);
  }
  .tags-error {
    color: var(--color-red, #c44);
  }

  .path-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .path {
    color: color-mix(in srgb, var(--color-foreground) 38%, transparent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }

  .ghost {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .ghost:hover {
    color: var(--color-foreground);
  }
</style>
