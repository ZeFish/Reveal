<script>
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
</script>

<div class="story-composer">
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
</style>
