<script>
  import { storyTheme, colorScheme, contrastInk, getFontFamilyWithFallback } from "$lib/story-theme.svelte.js";
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

  // A theme only stores its dark tokens — light mode is the derived swap
  // (contrastInk), same as +page.svelte's own app-chrome mood. This preview
  // is what the Garden page would show live, so it needs to track the
  // SAME system appearance the rest of the app follows, not just render
  // the theme's dark half unconditionally regardless of it.
  let bg = $derived(
    !storyTheme.darkBackground
      ? null
      : colorScheme.prefersDark
        ? storyTheme.darkBackground
        : contrastInk(storyTheme.darkBackground)
  );
  let fgColor = $derived(
    !storyTheme.darkBackground
      ? null
      : colorScheme.prefersDark
        ? contrastInk(storyTheme.darkBackground)
        : storyTheme.darkBackground
  );
  let accentColor = $derived(storyTheme.darkAccent);
  let fontH = $derived(getFontFamilyWithFallback(storyTheme.fontHeader, true));
  let fontT = $derived(getFontFamilyWithFallback(storyTheme.fontText, false));
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
    /* Two straight guesses at a matching surface token (--color-surface,
       then --color-surface-high) each painted a visibly different flat
       color than the sidebar nav — something about how this wrapper
       inherits those tokens doesn't match nav's own computed value.
       CullView's canvas has no seam against the nav at all because it
       paints nothing of its own and just shows .cull's real background
       through — mirroring that (transparent when unthemed, same as
       StoryComposer's own .composer below it) instead of guessing again. */
    background: var(--theme-bg, transparent);
    transition: background 0.3s var(--ease-standard);
  }
</style>
