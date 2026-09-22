<script>
  // Scopes for the frame currently on screen — histogram, luma waveform, RGB
  // parade and vectorscope. Every number comes from DevelopView's own single
  // pixel read (see its getSourcePixelData/updateHistogram); this component
  // only draws.
  //
  // `histogram` is relayed to a DETACHED dev panel too (+page.svelte's
  // sendDevStateToPanel), `scopes` is not — it's ~130k numbers, fine as a
  // prop inside one window, absurd through a JSON event per slider tick. So
  // the waveform modes are skipped when there's no scope data rather than
  // drawn empty; a detached panel just cycles the two histogram modes.
  let { histogram = null, scopes = null } = $props();

  /** @type {HTMLCanvasElement | null} */
  let canvasEl = $state(null);
  /** @type {HTMLDivElement | null} */
  let containerEl = $state(null);

  const MODES = /** @type {const} */ (["rgb", "luma", "waveform", "parade", "vector"]);
  const MODE_LABEL = {
    rgb: "RGB histogram",
    luma: "Luminance histogram",
    waveform: "Luma waveform",
    parade: "RGB parade",
    vector: "Vectorscope",
  };
  let modeIndex = $state(0);
  let mode = $derived(MODES[modeIndex]);
  // The histograms read fine in a 44px strip; a waveform or a vectorscope
  // needs real vertical room to say anything, so the widget grows into it.
  let tall = $derived(mode !== "rgb" && mode !== "luma");

  function cycleMode() {
    let next = (modeIndex + 1) % MODES.length;
    // Without scope data (detached panel) the three scope modes would draw
    // nothing — skip straight back to the histograms instead of cycling
    // through three blank frames.
    if (!scopes) while (MODES[next] !== "rgb" && MODES[next] !== "luma") next = (next + 1) % MODES.length;
    modeIndex = next;
  }

  $effect(() => {
    draw(histogram, scopes, mode);
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

  /** @param {Uint32Array} a */
  const maxOf = (a) => a.reduce((m, v) => (v > m ? v : m), 1);

  /**
   * @param {{r: Uint32Array, g: Uint32Array, b: Uint32Array, luma: Uint32Array} | null} hist
   * @param {any} sc
   * @param {string} m
   */
  function draw(hist, sc, m) {
    if (!canvasEl) return;
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    // Theme colors, not hardcoded ones — read off the container so each
    // theme's own --color-red/-green/-blue (and fg/bg for mono) apply.
    const style = getComputedStyle(containerEl ?? canvasEl);
    /** @param {string} name @param {string} fallback */
    const read = (name, fallback) => style.getPropertyValue(name).trim() || fallback;
    const RED = resolveRGB(read("--color-red", "#ff3232"));
    const GREEN = resolveRGB(read("--color-green", "#32ff32"));
    const BLUE = resolveRGB(read("--color-blue", "#326eff"));
    const FG = resolveRGB(read("--color-foreground", "#fff"));

    if (m === "rgb" || m === "luma") {
      drawHistogram(ctx, hist, m === "luma", { RED, GREEN, BLUE, FG });
    } else if (sc) {
      if (m === "vector") drawVectorscope(ctx, sc);
      else drawWaveform(ctx, sc, m === "parade", { RED, GREEN, BLUE, FG });
    }
  }

  const HIST_W = 256;
  const HIST_H = 44;

  /**
   * @param {CanvasRenderingContext2D} ctx
   * @param {{r: Uint32Array, g: Uint32Array, b: Uint32Array, luma: Uint32Array} | null} hist
   * @param {boolean} isMono
   * @param {{RED: number[], GREEN: number[], BLUE: number[], FG: number[]}} colors
   */
  function drawHistogram(ctx, hist, isMono, { RED, GREEN, BLUE, FG }) {
    const W = HIST_W;
    const H = HIST_H;
    ctx.canvas.width = W;
    ctx.canvas.height = H;
    ctx.clearRect(0, 0, W, H);
    if (!hist) return;

    ctx.globalCompositeOperation = "lighter";
    /** @param {Uint32Array} channel @param {number[]} rgb @param {number} max */
    const plot = (channel, [r, g, b], max) => {
      const scale = H / Math.log1p(max);
      /** @param {number} x */
      const y = (x) => H - Math.log1p(channel[x]) * scale;

      // A flat fill opacity lets pure white/saturated highlights dominate the
      // eye even when every channel is present — a bottom-heavy gradient (20%
      // near the baseline, 5% near the peak) keeps the shape legible without
      // that glare, and the stroke on top still marks exactly where peaks are.
      const gradient = ctx.createLinearGradient(0, 0, 0, H);
      gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, 0.05)`);
      gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0.2)`);
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
      plot(hist.luma, FG, maxOf(hist.luma));
    } else {
      const max = Math.max(maxOf(hist.r), maxOf(hist.g), maxOf(hist.b));
      plot(hist.r, RED, max);
      plot(hist.g, GREEN, max);
      plot(hist.b, BLUE, max);
    }
    ctx.globalCompositeOperation = "source-over";
  }

  /**
   * A waveform is a per-column value distribution, so it's painted pixel by
   * pixel into an ImageData rather than stroked — that's what gives it the
   * density falloff (how MANY pixels sit at this value in this column) a
   * path can't express.
   * @param {CanvasRenderingContext2D} ctx
   * @param {any} sc
   * @param {boolean} isParade
   * @param {{RED: number[], GREEN: number[], BLUE: number[], FG: number[]}} colors
   */
  function drawWaveform(ctx, sc, isParade, { RED, GREEN, BLUE, FG }) {
    const cols = sc.cols;
    const levels = sc.levels;
    const W = cols;
    const H = levels;
    ctx.canvas.width = W;
    ctx.canvas.height = H;
    const out = ctx.createImageData(W, H);
    const px = out.data;

    /**
     * @param {Uint32Array} buf
     * @param {number[]} rgb
     * @param {number} xOffset destination column offset (parade panels)
     * @param {number} xScale source columns per destination column
     */
    const paint = (buf, [cr, cg, cb], xOffset, xScale) => {
      // One shared ceiling per channel: a per-column normalization would make
      // an empty sky read as loud as the subject.
      const max = maxOf(buf);
      const norm = 255 / Math.log1p(max);
      const dstCols = Math.round(cols / xScale);
      for (let dx = 0; dx < dstCols; dx++) {
        const sx = Math.min(cols - 1, (dx * xScale) | 0);
        const base = sx * levels;
        for (let v = 0; v < levels; v++) {
          const count = buf[base + v];
          if (!count) continue;
          const a = Math.min(255, Math.log1p(count) * norm);
          // level 0 is black, so it belongs at the BOTTOM of the scope
          const i = ((levels - 1 - v) * W + xOffset + dx) * 4;
          px[i] = Math.min(255, px[i] + ((cr * a) / 255));
          px[i + 1] = Math.min(255, px[i + 1] + ((cg * a) / 255));
          px[i + 2] = Math.min(255, px[i + 2] + ((cb * a) / 255));
          px[i + 3] = Math.min(255, px[i + 3] + a);
        }
      }
    };

    if (isParade) {
      const third = Math.floor(W / 3);
      paint(sc.r, RED, 0, cols / third);
      paint(sc.g, GREEN, third, cols / third);
      paint(sc.b, BLUE, third * 2, cols / third);
    } else {
      paint(sc.luma, FG, 0, 1);
    }
    ctx.putImageData(out, 0, 0);
  }

  /**
   * @param {CanvasRenderingContext2D} ctx
   * @param {any} sc
   */
  function drawVectorscope(ctx, sc) {
    const S = sc.vecSize;
    ctx.canvas.width = S;
    ctx.canvas.height = S;
    const buf = sc.vector;
    const out = ctx.createImageData(S, S);
    const px = out.data;
    const max = maxOf(buf);
    const norm = 255 / Math.log1p(max);
    const half = S / 2;

    for (let y = 0; y < S; y++) {
      for (let x = 0; x < S; x++) {
        const count = buf[y * S + x];
        if (!count) continue;
        const a = Math.min(255, Math.log1p(count) * norm);
        // Color each point by the hue its own position represents — that's
        // what makes a vectorscope readable at a glance (where the reds sit,
        // how far out the saturation runs) instead of a grey blob.
        const cb = ((x + 0.5) / S) * 2 - 1;
        const cr = 1 - ((y + 0.5) / S) * 2;
        const hue = Math.atan2(cr, cb);
        const [r, g, b] = hueToRGB(hue);
        const i = (y * S + x) * 4;
        px[i] = (r * a) / 255;
        px[i + 1] = (g * a) / 255;
        px[i + 2] = (b * a) / 255;
        px[i + 3] = a;
      }
    }
    ctx.putImageData(out, 0, 0);

    // Graticule: the neutral axis and a 100%-saturation reference ring, so
    // "how far out is this" has something to be far from.
    ctx.strokeStyle = "rgba(255, 255, 255, 0.16)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.arc(half, half, half * 0.75, 0, Math.PI * 2);
    ctx.moveTo(half, 0);
    ctx.lineTo(half, S);
    ctx.moveTo(0, half);
    ctx.lineTo(S, half);
    ctx.stroke();
  }

  /** Full-saturation RGB for an angle in radians. @param {number} h */
  function hueToRGB(h) {
    const deg = ((h * 180) / Math.PI + 360) % 360;
    const c = 1;
    const x = 1 - Math.abs(((deg / 60) % 2) - 1);
    /** @type {number[]} */
    let rgb;
    if (deg < 60) rgb = [c, x, 0];
    else if (deg < 120) rgb = [x, c, 0];
    else if (deg < 180) rgb = [0, c, x];
    else if (deg < 240) rgb = [0, x, c];
    else if (deg < 300) rgb = [x, 0, c];
    else rgb = [c, 0, x];
    return rgb.map((v) => v * 255);
  }
</script>

<div
  class="scopes"
  class:mono={mode === "luma"}
  class:tall
  bind:this={containerEl}
  onclick={cycleMode}
  title={`${MODE_LABEL[mode]} — click to cycle`}
  role="button"
  tabindex="0"
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && cycleMode()}
>
  <canvas bind:this={canvasEl} class:square={mode === "vector"}></canvas>
  {#if tall}
    <span class="scope-label din">{MODE_LABEL[mode]}</span>
  {/if}
</div>

<style>
  .scopes {
    position: relative;
    width: 100%;
    height: 44px;
    border-radius: var(--radius-sm, 3px);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-foreground) 10%, transparent);
    overflow: hidden;
    cursor: pointer;
    transition: height var(--duration-fast, 0.12s) var(--ease-soft, ease);
  }
  .scopes.mono {
    background: var(--color-background);
  }
  /* A waveform/vectorscope is read as a shape in a dark field — the same 4%
     wash the histograms sit on greys out its own falloff. */
  .scopes.tall {
    height: 132px;
    background: #000;
  }
  canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
  /* A vectorscope's whole reading is angular — stretching it to the panel's
     aspect would turn its reference ring into an ellipse and every hue
     direction into a lie. */
  canvas.square {
    width: auto;
    aspect-ratio: 1;
    margin: 0 auto;
  }
  .scope-label {
    position: absolute;
    left: 6px;
    bottom: 4px;
    font-family: var(--font-header, sans-serif);
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
    pointer-events: none;
  }
</style>
