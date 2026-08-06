<script>
  import Icon from "$lib/components/Icon.svelte";

  let {
    showClipping = false,
    /** @type {() => void} */
    onCropClick = () => {},
    /** @type {() => void} */
    toggleClipping = () => {},
    /** @type {() => void} */
    onPresetClick = () => {},
    /** @type {() => void} */
    onExportDesktopClick = () => {},
  } = $props();
</script>

<div class="tools-strip">
  <button class="tool-btn" title="Recadrer (Crop)" onclick={onCropClick}>
    <Icon name="crop" size="14px" />
  </button>
  <button
    class="tool-btn clipping-btn"
    class:active={showClipping}
    title="Avertissement d'écrêtage (Blancs & Noirs)"
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
  <button class="tool-btn" title="Exporter sur le Bureau" onclick={onExportDesktopClick}>
    <Icon name="download-simple" size="14px" />
  </button>
</div>

<style>
  .tools-strip {
    display: flex;
    gap: 8px;
    padding: 0 16px 12px;
  }

  .tool-btn {
    all: unset;
    cursor: pointer;
    position: relative;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
    border: 1px solid var(--color-border);
    transition: color var(--duration-instant) var(--ease-soft), background var(--duration-instant) var(--ease-soft), border-color var(--duration-instant) var(--ease-soft);
  }

  .tool-btn:hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    border-color: color-mix(in srgb, var(--color-foreground) 20%, transparent);
  }

  .tool-btn.active {
    background: color-mix(in srgb, var(--color-foreground) 15%, transparent);
    border-color: var(--color-accent);
    color: var(--color-foreground);
  }

  .clip-dots {
    position: absolute;
    bottom: 2px;
    right: 3px;
    display: flex;
    gap: 2px;
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
