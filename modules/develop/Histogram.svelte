<script>
  // RGB histogram for the frame currently on screen — bin counts come from
  // DevelopView's own pixel read (see its getSourcePixelData/updateHistogram),
  // relayed down here the same way every other live dev-panel value is
  // (docked: a plain prop; detached: through main-dev-state — see
  // +page.svelte's sendDevStateToPanel). This component only draws.
  let { histogram = null } = $props();

  /** @type {HTMLCanvasElement | null} */
  let canvasEl = $state(null);
  /** @type {HTMLDivElement | null} */
  let containerEl = $state(null);

  // Click toggles RGB curves off in favor of a single luma curve in the
  // theme's own foreground/background — Francis: "pour l'instant au moins".
  let mono = $state(false);

  // Fixed logical resolution (one bin = one column); CSS stretches it to the
  // panel's actual width, same trick the app already uses for the photo mat.
  const W = 256;
  const H = 44;

  $effect(() => {
    draw(histogram, mono);
  });

  /**
   * @param {{r: Uint32Array, g: Uint32Array, b: Uint32Array, luma: Uint32Array} | null} hist
   * @param {boolean} isMono
   */
  function draw(hist, isMono) {
    if (!canvasEl) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;
    canvasEl.width = W;
    canvasEl.height = H;
    ctx.clearRect(0, 0, W, H);
    if (!hist) return;

    // Theme colors, not hardcoded ones — read off the container so each
    // theme's own --color-red/-green/-blue (and fg/bg for mono) apply. Canvas
    // fillStyle needs a resolved value, not the custom property itself.
    const style = getComputedStyle(containerEl ?? canvasEl);
    /** @param {string} name @param {string} fallback */
    const read = (name, fallback) => style.getPropertyValue(name).trim() || fallback;

    /** @param {Uint32Array} channel */
    const maxOf = (channel) => channel.reduce((m, v) => (v > m ? v : m), 1);

    ctx.globalCompositeOperation = "lighter";
    /** @param {Uint32Array} channel @param {string} color @param {number} max */
    const plot = (channel, color, max) => {
      const scale = H / Math.log1p(max);
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

    if (isMono) {
      ctx.globalAlpha = 0.85;
      plot(hist.luma, read("--color-foreground", "#fff"), maxOf(hist.luma));
      ctx.globalAlpha = 1;
    } else {
      const max = Math.max(maxOf(hist.r), maxOf(hist.g), maxOf(hist.b));
      ctx.globalAlpha = 0.8;
      plot(hist.r, read("--color-red", "#ff3232"), max);
      plot(hist.g, read("--color-green", "#32ff32"), max);
      plot(hist.b, read("--color-blue", "#326eff"), max);
      ctx.globalAlpha = 1;
    }
  }
</script>

<div
  class="histogram"
  class:mono
  bind:this={containerEl}
  onclick={() => (mono = !mono)}
  title={mono ? "Histogramme luminance — clic pour RVB" : "Histogramme RVB — clic pour luminance"}
  role="button"
  tabindex="0"
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (mono = !mono)}
>
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
    cursor: pointer;
  }
  .histogram.mono {
    background: var(--color-background);
  }
  canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
