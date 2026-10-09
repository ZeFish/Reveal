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
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: stretch;
    justify-content: stretch;
    cursor: zoom-out;
    background: var(--color-background);
    animation: fullscreen-fade-in var(--transition-fast) ease-out;
  }
  .fullscreen-photo :global(main) {
    width: 100%;
    height: 100%;
    max-width: 100vw;
    max-height: 100vh;
  }
</style>
