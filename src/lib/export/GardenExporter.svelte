<script>
  /** @type {{ storySet?: Set<unknown>, progress?: unknown, liveUrl?: string, publishStory?: () => void }} */
  let {
    storySet = new Set(),
    progress = null,
    liveUrl = "",
    publishStory = () => {},
  } = $props();
</script>

<div class="export-target-card">
  <h3>GARDEN PUBLISHING (CDN)</h3>
  <p class="desc">Publishes your story live on the Garden network with a high-speed CDN and responsive galleries.</p>
  {#if liveUrl}
    <p class="live-link">Live: <a href={liveUrl} target="_blank" rel="noreferrer">{liveUrl}</a></p>
  {/if}
  <div class="target-actions">
    <button class="publish-btn" onclick={publishStory} disabled={!!progress || !storySet.size}>
      Publish to Garden ({storySet.size} images)
    </button>
  </div>
</div>

<style>
  .export-target-card {
    padding: 1rem;
    background: var(--color-surface-dark-1);
    border-radius: 8px;
    border: var(--stroke-width) solid var(--color-border);
  }
  h3 {
    margin: 0 0 0.4rem;
  }
  .desc {
    margin: 0 0 0.8rem;
  }
  .live-link {
    margin: 0 0 1rem;
  }
  .publish-btn {
    padding: 0.4rem 1rem;
    cursor: pointer;
  }
  .publish-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
