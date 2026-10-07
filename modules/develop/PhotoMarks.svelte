<script>
  import { Icon } from "@modules/core";

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
   * @property {boolean} [comparing] the photo as shot is being shown
   * @property {() => void} [onCompareDown] before / after: pressed (a tap switches, a hold peeks)
   * @property {() => void} [onCompareUp] before / after: let go
   * @property {string} [checkLayer] the check layer on screen ("none" when there is none)
   * @property {() => void} [onToggleCheckLayer]
   * @property {boolean} [showCaption] the caption is shown under the photo
   * @property {() => void} [onToggleCaption]
   */

  /** @type {Props} */
  let {
    rating = 0,
    inStory = false,
    readOnly = false,
    onRate = () => {},
    onToggleStory = () => {},
    comparing = false,
    onCompareDown = () => {},
    onCompareUp = () => {},
    checkLayer = "none",
    onToggleCheckLayer = () => {},
    showCaption = false,
    onToggleCaption = () => {},
  } = $props();

  const CHECK_LAYER_NAMES = {
    clipping: "Clipping",
    false_color: "False Color",
    saturation: "Saturation",
    hue: "Hue",
    solar: "Solarize",
  };
  let checkLayerName = $derived(/** @type {Record<string, string>} */ (CHECK_LAYER_NAMES)[checkLayer] ?? "Off");
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
  <span class="sep" aria-hidden="true"></span>
  <!-- Before / after: a tap switches, a hold peeks and comes back on release (\ or Y). -->
  <button
    type="button"
    class="compare-btn ghost"
    class:on={comparing}
    aria-pressed={comparing}
    aria-label="Before / after"
    title="Before / after: tap to switch, hold to peek (\ or Y)"
    onpointerdown={(e) => { e.currentTarget.setPointerCapture?.(e.pointerId); onCompareDown(); }}
    onpointerup={() => onCompareUp()}
    onpointercancel={() => onCompareUp()}
    onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); e.stopPropagation(); if (!e.repeat) onCompareDown(); } }}
    onkeyup={(e) => { if (e.key === "Enter" || e.key === " ") { e.stopPropagation(); onCompareUp(); } }}
  >
    <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M12 4v16" />
      <path d="M3 4h9v16H3z" class="half" />
    </svg>
  </button>
  <span class="sep" aria-hidden="true"></span>
  <!-- Check layer: a tap turns it on or off; its modes are in the bar that opens above. -->
  <button
    type="button"
    class="tool-btn ghost"
    class:on={checkLayer !== "none"}
    aria-pressed={checkLayer !== "none"}
    aria-label="Check layer"
    title={`Check layer (${checkLayerName}): clipping, false colour, saturation, hue`}
    onclick={() => onToggleCheckLayer()}
  >
    <Icon name="circle-half" size="var(--icon-sm)" />
    {#if checkLayer !== "none"}<span class="clip-indicator {checkLayer}"></span>{/if}
  </button>
  <button
    type="button"
    class="tool-btn ghost"
    class:on={showCaption}
    aria-pressed={showCaption}
    aria-label="Caption"
    title="Show the caption under the photo"
    onclick={() => onToggleCaption()}
  >
    <Icon name="subtitles" size="var(--icon-sm)" />
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
  .story-btn svg,
  .compare-btn svg {
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
  .tool-btn {
    position: relative;
  }
  .tool-btn.on,
  .compare-btn.on {
    color: var(--color-accent);
  }
  .clip-indicator {
    position: absolute;
    bottom: 2px;
    right: 2px;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-accent);

    &.clipping { background: #ef4444; }
    &.false_color { background: #22c55e; }
    &.saturation { background: #ec4899; }
    &.hue { background: #06b6d4; }
    &.solar { background: #e2e8f0; }
  }
  .compare-btn.on {
    color: var(--color-accent);
  }
  .compare-btn .half {
    fill: currentColor;
    fill-opacity: 0.25;
    stroke: none;
  }
  .sep {
    width: 1px;
    height: 12px;
    background: var(--color-border);
  }
</style>
