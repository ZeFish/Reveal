<script>
  /**
   * @typedef {Object} Props
   * @property {boolean} active
   * @property {{ x: number, y: number }} pos
   * @property {{ x: number, y: number }} [norm]
   * @property {{ el: any, w: number, h: number } | null} [source]
   * @property {number} [diameter]
   * @property {number} [zoom]
   */

  /** @type {Props} */
  let {
    active = false,
    pos = { x: 0, y: 0 },
    norm = { x: 0.5, y: 0.5 },
    source = null,
    diameter = 220,
    zoom = 2,
  } = $props();

  /** @type {HTMLCanvasElement | null} */
  let canvasEl = $state(null);

  export function drawLoupe() {
    if (!active || !source || !canvasEl) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;
    canvasEl.width = diameter;
    canvasEl.height = diameter;
    const span = diameter / zoom;
    const sx = norm.x * source.w - span / 2;
    const sy = norm.y * source.h - span / 2;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, diameter, diameter);
    ctx.drawImage(source.el, sx, sy, span, span, 0, 0, diameter, diameter);
  }

  $effect(() => {
    if (active && canvasEl && source) {
      drawLoupe();
    }
  });
</script>

{#if active}
  <div class="loupe" style="left: {pos.x}px; top: {pos.y}px; width: {diameter}px; height: {diameter}px;">
    <canvas bind:this={canvasEl} width={diameter} height={diameter}></canvas>
    <div class="loupe-crosshair"></div>
  </div>
{/if}

<style>
  .loupe {
    position: fixed;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    overflow: hidden;
    pointer-events: none;
    z-index: 50;
    border: 2px solid rgba(255, 255, 255, 0.85);
    box-shadow: var(--shadow-hover);
    background: var(--color-surface-raised);
  }
  .loupe canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
  .loupe-crosshair {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .loupe-crosshair::before,
  .loupe-crosshair::after {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    background: rgba(255, 255, 255, 0.7);
    box-shadow: 0 0 1px rgba(0, 0, 0, 0.6);
  }
  .loupe-crosshair::before {
    width: 1px;
    height: 11px;
    transform: translate(-50%, -50%);
  }
  .loupe-crosshair::after {
    width: 11px;
    height: 1px;
    transform: translate(-50%, -50%);
  }
</style>
