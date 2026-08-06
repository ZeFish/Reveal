<script>
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
  import { extractGardenUrl } from "$lib/story.js";
  import StoryComposer from "./StoryComposer.svelte";

  let {
    view = [],
    storySet = new Set(),
    storyContent = "",
    progress = null,
    liveUrl = "",
    thumbUrl = /** @type {(path: string, version: number) => string} */ ((p) => p),
    saveStoryContent = () => {},
  } = $props();

  let activeGardenUrl = $derived(extractGardenUrl(storyContent) || liveUrl);

  async function openGardenPage(url) {
    if (!url) return;
    if (isTauri) {
      await invoke("open_path", { path: url });
    } else {
      window.open(url, "_blank");
    }
  }
</script>

<div class="story-composer">
  <div class="composer-header">
    <h2>COMPOSER L'HISTOIRE</h2>
    <div class="composer-actions">
      {#if progress && (progress.verb === "publication" || progress.phase)}
        <span class="composer-status">
          {progress.phase || "Publication"}… {progress.done}/{progress.total}
        </span>
      {:else if activeGardenUrl}
        <span class="composer-status ok">Publié ✓ — {activeGardenUrl}</span>
        <button class="open-page-btn" onclick={() => openGardenPage(activeGardenUrl)}>
          OUVRIR LA PAGE ↗
        </button>
      {/if}
    </div>
  </div>

  <StoryComposer
    content={storyContent}
    frames={view}
    {storySet}
    {thumbUrl}
    onSave={saveStoryContent}
  />
</div>

<style>
  .story-composer {
    flex: 1;
    min-height: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .composer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
  }
  .composer-header h2 {
    font-family: var(--font-header, sans-serif);
    font-size: 0.9rem;
    letter-spacing: 0.12em;
    margin: 0;
  }
  .composer-actions {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .composer-status {
    font-family: var(--font-monospace, monospace);
    font-size: 0.78rem;
    opacity: 0.7;
  }
  .composer-status.ok {
    color: #4caf50;
    opacity: 1;
  }
  .open-page-btn {
    all: unset;
    cursor: pointer;
    box-sizing: border-box;
    font-family: var(--font-header, sans-serif);
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    padding: 0.3rem 0.7rem;
    background: var(--color-surface-high, #27272a);
    border: 1px solid var(--color-accent);
    color: var(--color-accent);
    border-radius: 999px;
    transition: all 0.15s var(--ease-standard);
  }
  .open-page-btn:hover {
    background: var(--color-accent);
    color: #ffffff;
  }
</style>
