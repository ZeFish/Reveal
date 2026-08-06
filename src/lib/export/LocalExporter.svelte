<script>
  /** @type {{ storySet?: Set<unknown>, progress?: unknown, exportLocalStory?: () => void }} */
  let {
    storySet = new Set(),
    progress = null,
    exportLocalStory = () => {},
  } = $props();
</script>

<div class="export-target-card">
  <h3>EXPORTATION LOCAL (HTML / PDF)</h3>
  <p class="desc">Génère un portfolio HTML auto-hébergé avec des images optimisées et mise en page réactive.</p>
  <div class="target-actions">
    <button class="publish-btn" onclick={exportLocalStory} disabled={!!progress || !storySet.size}>
      Exporter ({storySet.size} images)
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
