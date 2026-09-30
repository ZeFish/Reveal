<script>
  import Icon from "$lib/components/Icon.svelte";

  /**
   * @typedef {Object} Props
   * @property {boolean} [showClipping]
   * @property {() => void} [onCropClick]
   * @property {() => void} [toggleClipping]
   * @property {() => void} [onPresetClick]
   * @property {() => void} [onExportDesktopClick]
   */

  /** @type {Props} */
  let {
    showClipping = false,
    onCropClick = () => {},
    toggleClipping = () => {},
    onPresetClick = () => {},
    onExportDesktopClick = () => {},
  } = $props();
</script>

<div class="tools-strip">
  <button class="tool-btn" title="Crop" onclick={onCropClick}>
    <Icon name="crop" size="14px" />
  </button>
  <button
    class="tool-btn clipping-btn"
    aria-pressed={showClipping}
    title="Clipping warning (highlights & shadows)"
    onclick={toggleClipping}
  >
    <Icon name="circle-half" size="14px" />
    {#if showClipping}
      <span class="clip-dots">
        <span class="dot red"></span>
        <span class="dot blue"></span>
      </span>
    {/if}
  </button>
  <button class="tool-btn" title="Presets" onclick={onPresetClick}>
    <Icon name="sliders-horizontal" size="14px" />
  </button>
  <button class="tool-btn" title="Export to Desktop" onclick={onExportDesktopClick}>
    <Icon name="download-simple" size="14px" />
  </button>
</div>

<style>
  .tools-strip {
    display: flex;
    gap: var(--space-d2);
    padding: 0 var(--space) 12px;
  }

  .tool-btn {
    cursor: pointer;
    position: relative;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
  }

  .clip-dots {
    position: absolute;
    bottom: 2px;
    right: 3px;
    display: flex;
    gap: var(--space-d8);
  }
  .dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
  }
  .dot.red {
    background: #ff3b30;
  }
  .dot.blue {
    background: #007aff;
  }
</style>
