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
    padding: var(--space);
    background: var(--color-surface-dark-1);
    border-radius: var(--radius-lg);
    border: var(--stroke-width) solid var(--color-border);
  }
  h3 {
    margin: 0 0 calc(var(--space-d4) * 2);
  }
  .desc {
    margin: 0 0 var(--space);
  }
  .publish-btn {
    padding: calc(var(--space-d4) * 2) var(--space);
    cursor: pointer;
  }
  .publish-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
