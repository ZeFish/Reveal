<script>
  // The photo's stars and its place in the quick collection, shown while developing — the
  // same two marks the grid cell carries, here also clickable. Both answer at once; the
  // write to disk follows (see `rate` and `toggleStoryWithPath`).
  /**
   * @typedef {Object} Props
   * @property {number} [rating]
   * @property {boolean} [inStory]
   * @property {boolean} [readOnly]
   * @property {(n: number) => void} [onRate]
   * @property {() => void} [onToggleStory]
   */

  /** @type {Props} */
  let { rating = 0, inStory = false, readOnly = false, onRate = () => {}, onToggleStory = () => {} } = $props();
</script>

<div class="marks hud" role="group" aria-label="Rating and quick collection">
  <div class="stars" role="radiogroup" aria-label="Rating">
    {#each [1, 2, 3, 4, 5] as n}
      <button
        type="button"
        class="star-btn ghost"
        class:on={n <= rating}
        role="radio"
        aria-checked={n === rating}
        aria-label={`${n} star${n > 1 ? "s" : ""}`}
        title={n === rating ? "Clear the rating" : `${n} star${n > 1 ? "s" : ""} (${n})`}
        disabled={readOnly}
        onclick={() => onRate(n === rating ? 0 : n)}
      >
        <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
          <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z" />
        </svg>
      </button>
    {/each}
  </div>
  <span class="sep" aria-hidden="true"></span>
  <button
    type="button"
    class="story-btn ghost"
    class:on={inStory}
    aria-pressed={inStory}
    title={inStory ? "Remove from the quick collection (Q)" : "Add to the quick collection (Q)"}
    disabled={readOnly}
    onclick={() => onToggleStory()}
  >
    <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
      <path d="M6 3h12v18l-6-4.5L6 21z" />
    </svg>
  </button>
</div>

<style>
  .marks {
    pointer-events: auto;
    display: inline-flex;
    align-items: center;
    gap: var(--space-d3);
    padding: 0 var(--space-d3);
    min-height: 26px;
    -webkit-app-region: no-drag;
  }
  .stars {
    display: flex;
    gap: 0;
  }
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--color-subtle);
    cursor: pointer;
  }
  button:disabled {
    cursor: default;
    opacity: 0.4;
  }
  .star-btn svg,
  .story-btn svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linejoin: round;
  }
  .star-btn.on {
    color: var(--color-accent);
  }
  .star-btn.on svg {
    fill: var(--color-accent);
  }
  .story-btn.on {
    color: var(--color-accent);
  }
  .story-btn.on svg {
    fill: var(--color-accent);
  }
  .sep {
    width: 1px;
    height: 12px;
    background: var(--color-border);
  }
</style>
