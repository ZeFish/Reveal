<script>
  // RGB histogram for the frame currently on screen — bin counts come from
  // DevelopView's own pixel read (see its getSourcePixelData/updateHistogram),
  // relayed down here the same way every other live dev-panel value is
  // (docked: a plain prop; detached: through main-dev-state — see
  // +page.svelte's sendDevStateToPanel). This component only draws.
  let { histogram = null } = $props();

  /** @type {HTMLCanvasElement | null} */
  let canvasEl = $state(null);

  // Fixed logical resolution (one bin = one column); CSS stretches it to the
  // panel's actual width, same trick the app already uses for the photo mat.
  const W = 256;
  const H = 44;

  $effect(() => {
    draw(histogram);
  });

  /** @param {{r: Uint32Array, g: Uint32Array, b: Uint32Array, luma: Uint32Array} | null} hist */
  function draw(hist) {
    if (!canvasEl) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;
    canvasEl.width = W;
    canvasEl.height = H;
    ctx.clearRect(0, 0, W, H);
    if (!hist) return;

    // Log scale — a handful of pure-white/pure-black pixels shouldn't flatten
    // every midtone bar to invisible, which a linear scale does on most photos.
    const max = Math.max(1, ...hist.r, ...hist.g, ...hist.b);
    const scale = H / Math.log1p(max);

    ctx.globalCompositeOperation = "lighter";
    /** @param {Uint32Array} channel @param {string} color */
    const plot = (channel, color) => {
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.moveTo(0, H);
      for (let x = 0; x < W; x++) {
        ctx.lineTo(x, H - Math.log1p(channel[x]) * scale);
      }
      ctx.lineTo(W - 1, H);
      ctx.closePath();
      ctx.fill();
    };
    plot(hist.r, "rgba(255,50,50,0.8)");
    plot(hist.g, "rgba(50,255,50,0.8)");
    plot(hist.b, "rgba(50,110,255,0.8)");
  }
</script>

<div class="histogram">
  <canvas bind:this={canvasEl} width={W} height={H}></canvas>
</div>

<style>
  .histogram {
    width: 100%;
    height: 44px;
    border-radius: var(--radius-sm, 3px);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-foreground) 10%, transparent);
    overflow: hidden;
  }
  canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
