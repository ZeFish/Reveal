<script>
  /** @type {{ storySet?: Set<unknown>, progress?: unknown, exportLocalStory?: () => void }} */
  let {
    storySet = new Set(),
    progress = null,
    exportLocalStory = () => {},
  } = $props();
</script>

<div class="export-target-card">
  <h3>LOCAL EXPORT (HTML / PDF)</h3>
  <p class="desc">Generates a self-hosted HTML portfolio with optimized images and a responsive layout.</p>
  <div class="target-actions">
    <button class="publish-btn" onclick={exportLocalStory} disabled={!!progress || !storySet.size}>
      Export ({storySet.size} images)
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
