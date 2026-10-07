<script>
  import { renderCheckLayer } from "./developAnalysis.js";
  import { zoneColors } from "./themeColor.js";

  /**
   * @typedef {Object} Props
   * @property {string} [matStyle]
   * @property {string} [effectiveCheckLayer]
   * @property {'shadows' | 'midtones' | 'highlights' | null} [zoneMask]
   * @property {() => { width: number, height: number, data: ImageData } | null} getSourcePixelData
   * @property {number} [canvasVersion]
   * @property {((mode: string) => void) | Function} [onSelectCheckLayer]
   * @property {() => void} [onCloseZoneMask]
   */

  /** @type {Props} */
  let {
    matStyle = "",
    effectiveCheckLayer = "none",
    zoneMask = null,
    getSourcePixelData,
    canvasVersion = 0,
    onSelectCheckLayer = () => {},
    onCloseZoneMask = () => {},
  } = $props();

  /** @type {HTMLCanvasElement | null} */
  let clipCanvasEl = $state(null);

  export function renderOverlay() {
    if (effectiveCheckLayer === "none" || !clipCanvasEl) return;
    const source = getSourcePixelData();
    if (!source) return;
    const { width, height, data } = source;
    clipCanvasEl.width = width;
    clipCanvasEl.height = height;
    const ctx = clipCanvasEl.getContext("2d");
    if (!ctx) return;
    renderCheckLayer(data, effectiveCheckLayer, ctx, width, height, effectiveCheckLayer.startsWith("zone_") ? zoneColors() : undefined);
  }

  $effect(() => {
    canvasVersion;
    if (effectiveCheckLayer !== "none") {
      renderOverlay();
    }
  });
</script>

<canvas
  bind:this={clipCanvasEl}
  class="photo-mat clip-overlay"
  style={matStyle}
></canvas>

<div class="check-layer-hud hud">
  {#if zoneMask}
    <div class="hud-modes">
      <span class="hud-title">ZONE MASK</span>
      <span
        class="hud-pill active"
        style={zoneMask === 'shadows' ? 'color: var(--color-blue);' : zoneMask === 'midtones' ? 'color: var(--color-green);' : 'color: var(--color-red);'}
      >
        {zoneMask === 'shadows' ? 'Shadows' : zoneMask === 'midtones' ? 'Midtones' : 'Highlights'}
      </span>
      <button
        class="hud-close"
        onclick={onCloseZoneMask}
        title="Close the mask (M key)"
        aria-label="Close"
      >✕</button>
    </div>
    <div class="hud-legend">
      <span class="legend-item" style="color: var(--color-foreground);">
        Hover a zone tab, or press 'M', to lock or unlock the mask display.
      </span>
    </div>
  {:else}
    <div class="hud-modes">
      <span class="hud-title">CHECK LAYER</span>
      <button
        class="hud-pill"
        class:active={effectiveCheckLayer === "clipping"}
        onclick={() => onSelectCheckLayer("clipping")}
      >Clipping</button>
      <button
        class="hud-pill"
        class:active={effectiveCheckLayer === "false_color"}
        onclick={() => onSelectCheckLayer("false_color")}
      >False Color (IRE)</button>
      <button
        class="hud-pill"
        class:active={effectiveCheckLayer === "saturation"}
        onclick={() => onSelectCheckLayer("saturation")}
      >Saturation</button>
      <button
        class="hud-pill"
        class:active={effectiveCheckLayer === "hue"}
        onclick={() => onSelectCheckLayer("hue")}
      >Hue</button>
      <button
        class="hud-pill"
        class:active={effectiveCheckLayer === "solar"}
        onclick={() => onSelectCheckLayer("solar")}
      >Solarize</button>
      <button
        class="hud-close"
        onclick={() => onSelectCheckLayer("none")}
        title="Close the check layer"
        aria-label="Close"
      >✕</button>
    </div>

    <div class="hud-legend">
      {#if effectiveCheckLayer === "clipping"}
        <span class="legend-item red">● Highlights (&gt;98%)</span>
        <span class="legend-item blue">● Shadows (&lt;2%)</span>
      {:else if effectiveCheckLayer === "false_color"}
        <span class="legend-item" style="color: #c084fc;">● Black (0–2%)</span>
        <span class="legend-item" style="color: #60a5fa;">● Shadows</span>
        <span class="legend-item" style="color: #22d3ee;">● Detail</span>
        <span class="legend-item" style="color: #4ade80;">● Grey 18%</span>
        <span class="legend-item" style="color: #f472b6;">● Skin</span>
        <span class="legend-item" style="color: #facc15;">● Highlights</span>
        <span class="legend-item" style="color: #ef4444;">● Clipped (100%)</span>
      {:else if effectiveCheckLayer === "saturation"}
        <span class="legend-item" style="color: #94a3b8;">0% Neutral</span>
        <span class="sat-gradient-bar"></span>
        <span class="legend-item" style="color: #f97316;">100% Saturated</span>
        <span class="legend-item" style="color: #f43f5e;">● Oversaturated (&gt;85%)</span>
      {:else if effectiveCheckLayer === "hue"}
        <span class="legend-item" style="color: var(--color-foreground);">Normalized spectrum (L=50%, S=100%) — checks hue drift and continuity</span>
      {:else if effectiveCheckLayer === "solar"}
        <span class="legend-item" style="color: var(--color-foreground);">Solarization iso-lines — spots dust, flat areas and micro-contrast</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .clip-overlay {
    position: absolute;
    pointer-events: none;
    z-index: 5;
  }

  .check-layer-hud {
    position: absolute;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-d3);
    padding: var(--space-d3) var(--space-d2);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--color-background) 88%, transparent);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: var(--stroke-width) solid var(--color-border);
    box-shadow: var(--shadow-raised);
    pointer-events: auto;
    font-size: 0.72rem;
  }

  .hud-modes {
    display: flex;
    align-items: center;
    gap: var(--space-d4);
  }

  .hud-title {
    font-family: var(--font-monospace);
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--color-muted);
    margin-right: var(--space-d4);
  }

  .hud-pill {
    padding: 3px 8px;
    border-radius: calc(var(--radius) - 2px);
    border: var(--stroke-width) solid transparent;
    background: transparent;
    color: var(--color-muted);
    font-size: 0.72rem;
    cursor: pointer;
    transition: all var(--transition-fast);

    &:hover {
      background: var(--color-surface);
      color: var(--color-foreground);
    }

    &.active {
      background: var(--color-foreground);
      color: var(--color-background);
      font-weight: 600;
    }
  }

  .hud-close {
    padding: 3px 6px;
    border-radius: calc(var(--radius) - 2px);
    border: none;
    background: transparent;
    color: var(--color-muted);
    cursor: pointer;
    font-size: 0.75rem;

    &:hover {
      background: var(--color-surface);
      color: var(--color-foreground);
    }
  }

  .hud-legend {
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    font-size: 0.68rem;
    font-family: var(--font-monospace);
  }

  .sat-gradient-bar {
    width: 60px;
    height: 6px;
    border-radius: 3px;
    background: linear-gradient(to right, #475569, #38bdf8, #eab308, #ef4444);
    margin: 0 4px;
  }

  .legend-item.red {
    color: var(--color-red);
  }
  .legend-item.blue {
    color: var(--color-blue);
  }
</style>
