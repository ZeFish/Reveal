<script>
  import { storyTheme, contrastInk, getFontFamilyWithFallback } from "$lib/story-theme.svelte.js";
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

  let bg = $derived(storyTheme.darkBackground);
  let accentColor = $derived(storyTheme.darkAccent);
  let fontH = $derived(getFontFamilyWithFallback(storyTheme.fontHeader, true));
  let fontT = $derived(getFontFamilyWithFallback(storyTheme.fontText, false));
  let fgColor = $derived(bg ? contrastInk(bg) : null);
</script>

<div
  class="story-composer"
  style:--theme-bg={bg ?? undefined}
  style:--theme-accent={accentColor ?? undefined}
  style:--theme-text-color={fgColor ?? undefined}
  style:--theme-font-header={fontH}
  style:--theme-font-text={fontT}
>
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
    /* Matches the sidebar nav's own --color-surface-high exactly (not
       --color-surface, which is one step lighter) — anything less than an
       exact match reads as a seam once the canvas is large and empty. */
    background: var(--theme-bg, var(--color-surface-high));
    transition: background 0.3s var(--ease-standard);
  }
</style>
