<script>
  import GardenExporter from "./GardenExporter.svelte";
  import LocalExporter from "./LocalExporter.svelte";

  let {
    destination = "garden",
    storySet = new Set(),
    progress = null,
    liveUrl = "",
    publishStory = () => {},
    exportLocalStory = () => {},
  } = $props();
</script>

{#if destination === "garden"}
  <GardenExporter {storySet} {progress} {liveUrl} {publishStory} />
{:else}
  <LocalExporter {storySet} {progress} {exportLocalStory} />
{/if}
