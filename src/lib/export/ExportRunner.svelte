<script>
  import GardenExporter from "./GardenExporter.svelte";
  import LocalExporter from "./LocalExporter.svelte";

  /**
   * @typedef {Object} Props
   * @property {string} [destination]
   * @property {Set<unknown>} [storySet]
   * @property {*} [progress]
   * @property {string} [liveUrl]
   * @property {() => void} [publishStory]
   * @property {() => void} [exportLocalStory]
   */

  /** @type {Props} */
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
