<script>
  import DevelopView from "../develop/DevelopView.svelte";

  /**
   * @typedef {Object} Props
   * @property {boolean} fullscreen
   * @property {any} [photo]
   * @property {string | null} [fullscreenUrl]
   * @property {number} [developPhotoPercent]
   * @property {boolean} [sourceOffline]
   * @property {() => void} [onExit]
   */

  /** @type {Props} */
  let {
    fullscreen = false,
    photo = null,
    fullscreenUrl = null,
    developPhotoPercent = 90,
    sourceOffline = false,
    onExit = () => {},
  } = $props();

  const stars = (/** @type {number | null | undefined} */ n) => "★".repeat(Math.max(0, Math.min(5, n || 0)));
</script>

{#if fullscreen && photo}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="fullscreen-photo" onclick={onExit} role="presentation">
    <!-- The real single-photo viewer, so fullscreen is identical to dev mode
         — including the same developPhotoPercent (the size slider in DevTab),
         so a photo sized to overflow its dev-panel frame overflows here too.
         Fullscreen just shows the pre-developed `fullscreenUrl`. -->
    <DevelopView
      picked={photo.name}
      imgUrl={fullscreenUrl ?? undefined}
      useCanvas={false}
      zoomMode="frame"
      {developPhotoPercent}
      {sourceOffline}
    />
    {#if photo.rating}
      <span class="fullscreen-rating">{stars(photo.rating)}</span>
    {/if}
  </div>
{/if}

<style>
  @keyframes fullscreen-fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .fullscreen-photo {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: zoom-out;
    background: var(--color-background);
    animation: fullscreen-fade-in var(--transition-fast) ease-out;
  }
  .fullscreen-rating {
    position: absolute;
    bottom: var(--space);
    left: 50%;
    transform: translateX(-50%);
    color: var(--color-accent);
    font-size: 14px;
    letter-spacing: 2px;
    pointer-events: none;
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.6);
  }
</style>
