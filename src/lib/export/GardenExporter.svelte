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
    background: var(--color-surface-low);
    border-radius: 8px;
    border: 1px solid var(--color-border);
  }
  h3 {
    font-family: var(--font-header, sans-serif);
    font-size: 0.85rem;
    margin: 0 0 0.4rem;
  }
  .desc {
    font-size: 0.75rem;
    color: var(--color-muted);
    margin: 0 0 0.8rem;
  }
  .live-link {
    font-family: var(--font-monospace, monospace);
    font-size: 0.7rem;
    color: var(--color-accent);
    margin: 0 0 1rem;
  }
  .publish-btn {
    color: var(--color-accent);
    border: 1px solid var(--color-accent);
    background: transparent;
    padding: 0.4rem 1rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .publish-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
