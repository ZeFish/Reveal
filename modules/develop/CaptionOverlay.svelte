<script>
  /**
   * @typedef {Object} Props
   * @property {string} caption
   */

  /** @type {Props} */
  let { caption = "" } = $props();

  let captionHtml = $derived(
    caption
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/\*(.+?)\*/g, "<em>$1</em>")
  );
</script>

<div class="caption-plate">
  <div class="caption-bar">{@html captionHtml}</div>
</div>

<style>
  .caption-plate {
    flex-shrink: 0;
    max-width: 90%;
    display: flex;
    justify-content: center;
  }
  .caption-bar {
    width: auto;
    max-width: 46rem;
    box-sizing: border-box;
    padding: 0 var(--space-d4);
    text-align: center;
    white-space: pre-wrap;
  }
</style>
