<script>
  import { emit } from "@tauri-apps/api/event";
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
    photoPath,
  } = $props();

  function captionEdited() {
    emit("dev-panel-caption-updated", { caption });
  }

  function copyPath() {
    if (photoPath) navigator.clipboard?.writeText(photoPath);
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
    <span class="mono dim">{exif.width} × {exif.height} px · RAW + preview</span>
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
    oninput={captionEdited}
  ></textarea>
  {#if photoPath}
    <div class="path-row">
      <span class="mono path">{parentDir(photoPath)}</span>
      <button class="ghost" onclick={copyPath} title="Copy path">
        <Icon name="copy" size="10px" />
      </button>
      <button class="ghost" onclick={revealInFinder} title="Reveal in Finder">
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
