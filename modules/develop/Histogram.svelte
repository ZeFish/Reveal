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
  // theme's own foreground/background — Francis: "for now at least".
  let mono = $state(false);

  // Fixed logical resolution (one bin = one column); CSS stretches it to the
  // panel's actual width, same trick the app already uses for the photo mat.
  const W = 256;
  const H = 44;

  $effect(() => {
    draw(histogram, mono);
  });

  // Canvas fillStyle parses ANY valid CSS color (hex, oklch(), whatever a
  // theme happens to use) — cheaper and more robust than hand-parsing the
  // custom property's raw string ourselves.
  /** @type {HTMLCanvasElement} */
  let probe;
  /** @param {string} colorStr @returns {[number, number, number]} */
  function resolveRGB(colorStr) {
    probe ??= document.createElement("canvas");
    probe.width = 1;
    probe.height = 1;
    const pctx = /** @type {CanvasRenderingContext2D} */ (probe.getContext("2d"));
    pctx.clearRect(0, 0, 1, 1);
    pctx.fillStyle = colorStr;
    pctx.fillRect(0, 0, 1, 1);
    const [r, g, b] = pctx.getImageData(0, 0, 1, 1).data;
    return [r, g, b];
  }

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
    // theme's own --color-red/-green/-blue (and fg/bg for mono) apply.
    const style = getComputedStyle(containerEl ?? canvasEl);
    /** @param {string} name @param {string} fallback */
    const read = (name, fallback) => style.getPropertyValue(name).trim() || fallback;

    /** @param {Uint32Array} channel */
    const maxOf = (channel) => channel.reduce((m, v) => (v > m ? v : m), 1);

    ctx.globalCompositeOperation = "lighter";
    /** @param {Uint32Array} channel @param {[number, number, number]} rgb @param {number} max */
    const plot = (channel, [r, g, b], max) => {
      const scale = H / Math.log1p(max);
      /** @param {number} x */
      const y = (x) => H - Math.log1p(channel[x]) * scale;

      // A flat fill opacity lets pure white/saturated highlights dominate the
      // eye even when every channel is present — a bottom-heavy gradient (50%
      // near the baseline, 25% near the peak) keeps the shape legible without
      // that glare, and the stroke on top still marks exactly where peaks are.
      const gradient = ctx.createLinearGradient(0, 0, 0, H);
      gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, 0.25)`);
      gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0.5)`);
      ctx.fillStyle = gradient;
      ctx.beginPath();
      ctx.moveTo(0, H);
      for (let x = 0; x < W; x++) ctx.lineTo(x, y(x));
      ctx.lineTo(W - 1, H);
      ctx.closePath();
      ctx.fill();

      ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, 0.9)`;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, y(0));
      for (let x = 1; x < W; x++) ctx.lineTo(x, y(x));
      ctx.stroke();
    };

    if (isMono) {
      plot(hist.luma, resolveRGB(read("--color-foreground", "#fff")), maxOf(hist.luma));
    } else {
      const max = Math.max(maxOf(hist.r), maxOf(hist.g), maxOf(hist.b));
      plot(hist.r, resolveRGB(read("--color-red", "#ff3232")), max);
      plot(hist.g, resolveRGB(read("--color-green", "#32ff32")), max);
      plot(hist.b, resolveRGB(read("--color-blue", "#326eff")), max);
    }
  }
</script>

<div
  class="histogram"
  class:mono
  bind:this={containerEl}
  onclick={() => (mono = !mono)}
  title={mono ? "Luminance histogram — click for RGB" : "RGB histogram — click for luminance"}
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
