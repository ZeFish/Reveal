<script>
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";
  import { extractGardenUrl } from "$lib/story.js";
  import { storyTheme, contrastInk, getFontFamilyWithFallback, getStoryThemeName } from "$lib/story-theme.svelte.js";
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

  let bg = $derived(storyTheme.darkBackground);
  let accentColor = $derived(storyTheme.darkAccent);
  let fontH = $derived(getFontFamilyWithFallback(storyTheme.fontHeader, true));
  let fontT = $derived(getFontFamilyWithFallback(storyTheme.fontText, false));
  let fgColor = $derived(bg ? contrastInk(bg) : null);
  let themeLabel = $derived(getStoryThemeName());

  /** @param {string} url */
  async function openGardenPage(url) {
    if (!url) return;
    if (isTauri) {
      await invoke("open_path", { path: url });
    } else {
      window.open(url, "_blank");
    }
  }
</script>

<div
  class="story-composer"
  style:--theme-bg={bg ?? undefined}
  style:--theme-accent={accentColor ?? undefined}
  style:--theme-text-color={fgColor ?? undefined}
  style:--theme-font-header={fontH}
  style:--theme-font-text={fontT}
>
  <div class="composer-header">
    <div class="header-main">
      <div class="title-wrap">
        <Icon name="note-pencil" size="14px" />
        <h2>VISUAL STORY</h2>
      </div>
      <div class="meta-pills">
        <span class="count-pill" title="Photos in this story">
          <Icon name="image" size="11px" />
          {storySet.size} photo{storySet.size > 1 ? "s" : ""}
        </span>
        {#if themeLabel}
          <span class="theme-pill" title="Active editorial theme">
            <span class="theme-pill-dot" style="background: var(--theme-accent, var(--color-accent));"></span>
            <span class="theme-pill-text">{themeLabel}</span>
          </span>
        {/if}
      </div>
    </div>

    <div class="composer-actions">
      {#if progress && (progress.verb === "publication" || progress.phase)}
        <div class="status-publishing">
          <span class="pulse-dot"></span>
          <span>{progress.phase || "Publication"}… {progress.done}/{progress.total}</span>
        </div>
      {:else if activeGardenUrl}
        <div class="status-live">
          <Icon name="check-circle" size="13px" />
          <span>En ligne</span>
        </div>
        <button class="open-page-btn" onclick={() => openGardenPage(activeGardenUrl)} title={activeGardenUrl}>
          <span>Voir sur Garden</span>
          <Icon name="arrow-square-out" size="11px" />
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
    background: var(--theme-bg, var(--color-background));
    transition: background 0.3s var(--ease-standard);
  }
  .composer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1.75rem;
    border-bottom: 1px solid var(--color-border);
    background: color-mix(in srgb, var(--theme-bg, var(--color-surface-low)) 60%, transparent);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    z-index: 5;
    transition: background 0.3s var(--ease-standard);
  }
  .header-main {
    display: flex;
    align-items: center;
    gap: 1.25rem;
  }
  .title-wrap {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--color-foreground);
  }
  .composer-header h2 {
    font-family: var(--theme-font-header, var(--font-header, sans-serif));
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    margin: 0;
    transition: font-family 0.25s var(--ease-standard);
  }
  .meta-pills {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .count-pill,
  .theme-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 0.7rem;
    color: var(--color-muted);
    background: var(--color-surface-low);
    border: 1px solid var(--color-border);
    padding: 2px 8px;
    border-radius: 999px;
  }
  .count-pill {
    font-family: var(--font-monospace, monospace);
  }
  .theme-pill {
    font-family: var(--theme-font-header, var(--font-header, sans-serif));
    letter-spacing: 0.04em;
  }
  .theme-pill-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    transition: background 0.3s var(--ease-standard);
  }

  .composer-actions {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .status-publishing {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-monospace, monospace);
    font-size: 0.75rem;
    color: var(--color-accent);
  }
  .pulse-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-accent);
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 0.3; transform: scale(0.85); }
    50% { opacity: 1; transform: scale(1.15); }
  }

  .status-live {
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: var(--font-header, sans-serif);
    font-size: 0.72rem;
    letter-spacing: 0.04em;
    color: #4ade80;
    background: rgba(74, 222, 128, 0.1);
    border: 1px solid rgba(74, 222, 128, 0.25);
    padding: 3px 8px;
    border-radius: 999px;
  }

  .open-page-btn {
    all: unset;
    cursor: pointer;
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-header, sans-serif);
    font-size: 0.72rem;
    letter-spacing: 0.05em;
    padding: 4px 12px;
    background: var(--color-surface-high);
    border: 1px solid var(--color-border);
    color: var(--color-foreground);
    border-radius: 999px;
    transition: all 0.15s var(--ease-standard);
  }
  .open-page-btn:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-surface-higher, var(--color-surface-high));
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  }
</style>
