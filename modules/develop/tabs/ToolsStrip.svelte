<script>
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
    effectiveCheckLayer === "clipping" ? "Écrêtage" :
    effectiveCheckLayer === "false_color" ? "False Color" :
    effectiveCheckLayer === "saturation" ? "Saturation" :
    effectiveCheckLayer === "hue" ? "Teintes (Hue)" :
    effectiveCheckLayer === "solar" ? "Solarisation" : "Désactivé"
  );
</script>

<div class="tools-strip">
  <button class="tool-btn" title="Crop" onclick={onCropClick}>
    <Icon name="crop" size="14px" />
  </button>
  <div class="check-menu-wrap">
    <button
      class="tool-btn clipping-btn"
      class:active={effectiveCheckLayer !== "none"}
      aria-pressed={effectiveCheckLayer !== "none"}
      title={`Check Layer (${checkLayerTitle}) — Clic pour basculer, clic-droit pour choisir`}
      onclick={toggleClipping}
      oncontextmenu={(e) => { e.preventDefault(); showMenu = !showMenu; }}
    >
      <Icon name="circle-half" size="14px" />
      {#if effectiveCheckLayer !== "none"}
        <span class="clip-dots">
          {#if effectiveCheckLayer === "clipping"}
            <span class="dot red"></span>
            <span class="dot blue"></span>
          {:else if effectiveCheckLayer === "false_color"}
            <span class="dot green"></span>
          {:else if effectiveCheckLayer === "saturation"}
            <span class="dot magenta"></span>
          {:else if effectiveCheckLayer === "hue"}
            <span class="dot cyan"></span>
          {:else if effectiveCheckLayer === "solar"}
            <span class="dot white"></span>
          {/if}
        </span>
      {/if}
    </button>
    {#if showMenu}
      <div class="check-menu-popover card" role="menu">
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "none"} onclick={() => { onSelectCheckLayer("none"); showMenu = false; }}>Désactivé</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "clipping"} onclick={() => { onSelectCheckLayer("clipping"); showMenu = false; }}>Écrêtage (Hautes/Basses)</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "false_color"} onclick={() => { onSelectCheckLayer("false_color"); showMenu = false; }}>False Color (IRE Vidéo)</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "saturation"} onclick={() => { onSelectCheckLayer("saturation"); showMenu = false; }}>Masque de Saturation</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "hue"} onclick={() => { onSelectCheckLayer("hue"); showMenu = false; }}>Masque des Teintes (Hue)</button>
        <button class="check-menu-item" class:selected={effectiveCheckLayer === "solar"} onclick={() => { onSelectCheckLayer("solar"); showMenu = false; }}>Solarisation (Micro-contraste)</button>
      </div>
    {/if}
  </div>
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
      background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
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
  .dot.red { background: #ff3b30; }
  .dot.blue { background: #007aff; }
  .dot.green { background: #22c55e; }
  .dot.magenta { background: #ec4899; }
  .dot.cyan { background: #06b6d4; }
  .dot.white { background: #e2e8f0; }
</style>
