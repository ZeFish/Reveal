<script>
  import { CHECK_COLORS, CHECK_SWATCH, rgbCss } from "../developAnalysis.js";
  import { Icon } from "@modules/core";

  /**
   * @typedef {Object} Props
   * @property {boolean} [showClipping]
   * @property {string} [checkLayer]
   * @property {() => void} [onCropClick]
   * @property {() => void} [toggleClipping]
   * @property {(mode: string) => void} [onSelectCheckLayer]
   * @property {() => void} [onPresetClick]
   * @property {() => void} [onExportDesktopClick]
   */

  /** @type {Props} */
  let {
    showClipping = false,
    checkLayer = "none",
    onCropClick = () => {},
    toggleClipping = () => {},
    onSelectCheckLayer = () => {},
    onPresetClick = () => {},
    onExportDesktopClick = () => {},
  } = $props();

  let showMenu = $state(false);

  let effectiveCheckLayer = $derived(
    checkLayer !== "none" ? checkLayer : (showClipping ? "clipping" : "none")
  );

  let checkLayerTitle = $derived(
    effectiveCheckLayer === "clipping" ? "Clipping" :
    effectiveCheckLayer === "false_color" ? "False Color" :
    effectiveCheckLayer === "saturation" ? "Saturation" :
    effectiveCheckLayer === "hue" ? "Hue" :
    effectiveCheckLayer === "solar" ? "Solarize" : "Off"
  );
</script>

<div class="tools-strip">
  <button class="tool-btn" title="Crop" onclick={onCropClick}>
    <Icon name="crop" size="var(--icon-lg)" />
  </button>
  <div class="check-menu-wrap">
    <button
      class="tool-btn clipping-btn"
      class:active={effectiveCheckLayer !== "none"}
      aria-pressed={effectiveCheckLayer !== "none"}
      title={`Check Layer (${checkLayerTitle}) — click to toggle, right-click to choose`}
      onclick={toggleClipping}
      oncontextmenu={(e) => { e.preventDefault(); showMenu = !showMenu; }}
    >
      <Icon name="circle-half" size="var(--icon-lg)" />
      {#if effectiveCheckLayer !== "none"}
        <span class="clip-dots">
          {#if effectiveCheckLayer === "clipping"}
            <span class="dot" style:background={rgbCss(CHECK_COLORS.clipHighlights)}></span>
            <span class="dot" style:background={rgbCss(CHECK_COLORS.clipShadows)}></span>
          {:else if effectiveCheckLayer === "false_color"}
            <span class="dot" style:background={rgbCss(CHECK_SWATCH.false_color)}></span>
          {:else if effectiveCheckLayer === "saturation"}
            <span class="dot" style:background={rgbCss(CHECK_SWATCH.saturation)}></span>
          {:else if effectiveCheckLayer === "hue"}
            <span class="dot" style:background={rgbCss(CHECK_SWATCH.hue)}></span>
          {:else if effectiveCheckLayer === "solar"}
            <span class="dot" style:background={rgbCss(CHECK_SWATCH.solar)}></span>
          {/if}
        </span>
      {/if}
    </button>
    {#if showMenu}
      <div class="check-menu-popover card" role="menu">
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "none"} onclick={() => { onSelectCheckLayer("none"); showMenu = false; }}>Off</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "clipping"} onclick={() => { onSelectCheckLayer("clipping"); showMenu = false; }}>Clipping (Highlights/Shadows)</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "false_color"} onclick={() => { onSelectCheckLayer("false_color"); showMenu = false; }}>False Color (Video IRE)</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "saturation"} onclick={() => { onSelectCheckLayer("saturation"); showMenu = false; }}>Saturation mask</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "hue"} onclick={() => { onSelectCheckLayer("hue"); showMenu = false; }}>Hue mask</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "solar"} onclick={() => { onSelectCheckLayer("solar"); showMenu = false; }}>Solarization (micro-contrast)</button>
      </div>
    {/if}
  </div>
  <button class="tool-btn" title="Presets" onclick={onPresetClick}>
    <Icon name="sliders-horizontal" size="var(--icon-lg)" />
  </button>
  <button class="tool-btn" title="Export to Desktop" onclick={onExportDesktopClick}>
    <Icon name="download-simple" size="var(--icon-lg)" />
  </button>
</div>

<style>
  .tools-strip {
    display: flex;
    gap: var(--space-d2);
  }

  .check-menu-wrap {
    position: relative;
    display: inline-flex;
  }

  .check-menu-popover {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 50;
    min-width: 190px;
    padding: var(--space-d4);
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--color-surface);
    box-shadow: var(--shadow-raised);
    border-radius: var(--radius);
  }

  .check-menu-item {
    text-align: left;
    padding: 5px 8px;
    font-size: 0.72rem;
    border-radius: calc(var(--radius) - 2px);
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--color-muted);

    &:hover {
      background: var(--color-hover);
      color: var(--color-foreground);
    }
    &.selected {
      background: var(--color-foreground);
      color: var(--color-background);
      font-weight: 600;
    }
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
</style>
