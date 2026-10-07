/**
 * Pure image data analysis routines for Develop view:
 * - Histogram (RGB + Luma)
 * - Waveforms (R, G, B, Luma)
 * - Vectorscope (Cb, Cr)
 * - Check layer pixel overlays (clipping, false color IRE, saturation, hue, solar, zone masks)
 */

export const WF_COLS = 256;
export const WF_LEVELS = 128;
export const VEC_SIZE = 128;

/**
 * Creates reusable buffers for waveform and vectorscope analysis.
 */
export function createScopeAnalyzer() {
  const wfR = new Uint32Array(WF_COLS * WF_LEVELS);
  const wfG = new Uint32Array(WF_COLS * WF_LEVELS);
  const wfB = new Uint32Array(WF_COLS * WF_LEVELS);
  const wfL = new Uint32Array(WF_COLS * WF_LEVELS);
  const vec = new Uint32Array(VEC_SIZE * VEC_SIZE);

  /**
   * Compute histogram and scope buffers from raw RGBA pixel data.
   *
   * @param {{ width: number, height: number, data: Uint8ClampedArray | Uint8Array }} source
   * @returns {{ histogram: { r: Uint32Array, g: Uint32Array, b: Uint32Array, luma: Uint32Array }, scopes: { cols: number, levels: number, vecSize: number, r: Uint32Array, g: Uint32Array, b: Uint32Array, luma: Uint32Array, vector: Uint32Array } }}
   */
  function analyze(source) {
    const { width, height, data } = source;
    const r = new Uint32Array(256);
    const g = new Uint32Array(256);
    const b = new Uint32Array(256);
    const luma = new Uint32Array(256);

    wfR.fill(0);
    wfG.fill(0);
    wfB.fill(0);
    wfL.fill(0);
    vec.fill(0);

    const ROW_STRIDE = 3;
    for (let y = 0; y < height; y++) {
      const sampleRow = y % ROW_STRIDE === 0;
      let i = y * width * 4;
      for (let x = 0; x < width; x++, i += 4) {
        if (data[i + 3] < 10) continue;
        const rv = data[i];
        const gv = data[i + 1];
        const bv = data[i + 2];
        r[rv]++;
        g[gv]++;
        b[bv]++;

        const lv = (0.2126 * rv + 0.7152 * gv + 0.0722 * bv) | 0;
        luma[lv]++;

        if (!sampleRow) continue;
        const base = (((x * WF_COLS) / width) | 0) * WF_LEVELS;
        wfR[base + ((rv * WF_LEVELS) >> 8)]++;
        wfG[base + ((gv * WF_LEVELS) >> 8)]++;
        wfB[base + ((bv * WF_LEVELS) >> 8)]++;
        wfL[base + ((lv * WF_LEVELS) >> 8)]++;

        // BT.709 chroma difference, the same axes a broadcast vectorscope
        // plots: Cb right, Cr up. Both land in -128..127 for 8-bit RGB.
        const cb = -0.1146 * rv - 0.3854 * gv + 0.5 * bv;
        const cr = 0.5 * rv - 0.4542 * gv - 0.0458 * bv;
        const vx = (((cb + 128) * VEC_SIZE) / 256) | 0;
        const vy = (((128 - cr) * VEC_SIZE) / 256) | 0;
        vec[vy * VEC_SIZE + vx]++;
      }
    }

    return {
      histogram: { r, g, b, luma },
      scopes: {
        cols: WF_COLS,
        levels: WF_LEVELS,
        vecSize: VEC_SIZE,
        r: wfR,
        g: wfG,
        b: wfB,
        luma: wfL,
        vector: vec,
      },
    };
  }

  return { analyze };
}

/** @typedef {[number, number, number]} Rgb */

/**
 * The colours the check layers paint. This is the one source: the legend, the
 * toolbar dots and the photo marks read these, they never restate them.
 * RGB triplets, because the painter writes them straight into ImageData.
 */
export const CHECK_COLORS = {
  clipHighlights: /** @type {Rgb} */ ([255, 30, 30]),
  clipShadows: /** @type {Rgb} */ ([0, 120, 255]),
  /** Luminance bands of false colour, darkest first; `below` is the upper bound on Rec.709 luma (0-255). */
  falseColor: [
    { below: 6, rgb: [140, 20, 180], label: "Black (0–2%)" },
    { below: 26, rgb: [15, 60, 220], label: "Shadows" },
    { below: 56, rgb: [0, 175, 210], label: "Detail" },
    { below: 96, rgb: [65, 65, 65], label: null },
    { below: 122, rgb: [10, 210, 45], label: "Grey 18%" },
    { below: 148, rgb: [255, 130, 165], label: "Skin" },
    { below: 196, rgb: [140, 140, 140], label: null },
    { below: 232, rgb: [255, 225, 0], label: "Highlights" },
    { below: 250, rgb: [255, 115, 0], label: null },
    { below: Infinity, rgb: [255, 20, 20], label: "Clipped (100%)" },
  ],
  satStrong: /** @type {Rgb} */ ([255, 140, 20]),
  satOver: /** @type {Rgb} */ ([255, 0, 170]),
  /** Hue and solar paint a ramp or iso-lines, not one colour: these are their swatches. */
  hueSwatch: [0, 175, 210],
  solarSwatch: [226, 232, 240],
};

/** `[r,g,b]` → `rgb(r g b)` for CSS. */
/** @param {number[]} rgb */
export const rgbCss = ([r, g, b]) => `rgb(${r} ${g} ${b})`;

/** The colour that stands for a whole check layer in the toolbar and on photo marks. */
/** @type {Record<string, number[]>} */
export const CHECK_SWATCH = {
  clipping: CHECK_COLORS.clipHighlights,
  false_color: CHECK_COLORS.falseColor[4].rgb,
  saturation: CHECK_COLORS.satOver,
  hue: CHECK_COLORS.hueSwatch,
  solar: CHECK_COLORS.solarSwatch,
};

/** What the zone masks were painted with before they followed the theme. */
const DEFAULT_ZONE_COLORS = { shadows: [0, 130, 255], midtones: [30, 200, 70], highlights: [255, 40, 40] };

/**
 * Render visual check layer (clipping, false color, saturation, hue, solar, zone masks) onto a canvas context.
 *
 * @param {ImageData} sourceData
 * @param {string} checkLayer
 * @param {CanvasRenderingContext2D} ctx
 * @param {number} width
 * @param {number} height
 * @param {{ shadows: number[], midtones: number[], highlights: number[] }} [zoneColors] the theme's zone colours; the defaults are the old fixed ones
 */
export function renderCheckLayer(sourceData, checkLayer, ctx, width, height, zoneColors = DEFAULT_ZONE_COLORS) {
  if (checkLayer === "none" || !ctx) return;

  const data = sourceData.data;
  const out = ctx.createImageData(width, height);
  const outData = out.data;

  if (checkLayer.startsWith("zone_")) {
    const isShadows = checkLayer === "zone_shadows";
    const isHighlights = checkLayer === "zone_highlights";

    for (let i = 0; i < data.length; i += 4) {
      const a = data[i + 3];
      if (a < 10) continue;
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      const yNorm = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
      const lum = Math.sqrt(Math.max(0, Math.min(1, yNorm)));

      const ws = Math.max(0, Math.min(1, 1.0 - 2.0 * lum));
      const wh = Math.max(0, Math.min(1, 2.0 * (lum - 0.5)));
      const wm = Math.max(0, 1.0 - ws - wh);

      const weight = isShadows ? ws : (isHighlights ? wh : wm);
      if (weight > 0.01) {
        const [cr, cg, cb] = isShadows ? zoneColors.shadows : isHighlights ? zoneColors.highlights : zoneColors.midtones;
        outData[i] = cr;
        outData[i + 1] = cg;
        outData[i + 2] = cb;
        outData[i + 3] = Math.round(weight * 210);
      }
    }
  } else if (checkLayer === "clipping") {
    for (let i = 0; i < data.length; i += 4) {
      const a = data[i + 3];
      if (a < 10) continue;
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      if (r >= 252 && g >= 252 && b >= 252) {
        [outData[i], outData[i + 1], outData[i + 2]] = CHECK_COLORS.clipHighlights;
        outData[i + 3] = 230;
      } else if (r <= 3 && g <= 3 && b <= 3) {
        [outData[i], outData[i + 1], outData[i + 2]] = CHECK_COLORS.clipShadows;
        outData[i + 3] = 230;
      }
    }
  } else if (checkLayer === "false_color") {
    for (let i = 0; i < data.length; i += 4) {
      const a = data[i + 3];
      if (a < 10) continue;
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      const y = 0.2126 * r + 0.7152 * g + 0.0722 * b;

      const band = CHECK_COLORS.falseColor.find((x) => y < x.below) ?? CHECK_COLORS.falseColor[CHECK_COLORS.falseColor.length - 1];
      [outData[i], outData[i + 1], outData[i + 2]] = band.rgb;
      outData[i + 3] = 255;
    }
  } else if (checkLayer === "saturation") {
    for (let i = 0; i < data.length; i += 4) {
      const a = data[i + 3];
      if (a < 10) continue;
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      const max = Math.max(r, g, b);
      const min = Math.min(r, g, b);
      const delta = max - min;
      const sat = max === 0 ? 0 : delta / max;

      if (sat > 0.85) {
        [outData[i], outData[i + 1], outData[i + 2]] = CHECK_COLORS.satOver;
      } else if (sat > 0.65) {
        const t = (sat - 0.65) / 0.20;
        outData[i] = 255; outData[i + 1] = Math.round((1 - t) * 140); outData[i + 2] = 20;
      } else if (sat > 0.40) {
        const t = (sat - 0.40) / 0.25;
        outData[i] = Math.round(180 + t * 75); outData[i + 1] = Math.round(160 - t * 20); outData[i + 2] = 40;
      } else if (sat > 0.15) {
        const v = Math.round(sat * 255);
        outData[i] = Math.round(v * 0.8); outData[i + 1] = Math.round(v * 0.9); outData[i + 2] = Math.round(v * 1.1);
      } else {
        const v = Math.round(sat * 200);
        outData[i] = v; outData[i + 1] = v; outData[i + 2] = v;
      }
      outData[i + 3] = 255;
    }
  } else if (checkLayer === "hue") {
    for (let i = 0; i < data.length; i += 4) {
      const a = data[i + 3];
      if (a < 10) continue;
      const r = data[i] / 255;
      const g = data[i + 1] / 255;
      const b = data[i + 2] / 255;
      const max = Math.max(r, g, b);
      const min = Math.min(r, g, b);
      const delta = max - min;

      if (delta < 0.04 || max < 0.04 || min > 0.96) {
        outData[i] = 110; outData[i + 1] = 110; outData[i + 2] = 110;
      } else {
        let h = 0;
        if (max === r) {
          h = ((g - b) / delta) % 6;
        } else if (max === g) {
          h = (b - r) / delta + 2;
        } else {
          h = (r - g) / delta + 4;
        }
        h = h / 6;
        if (h < 0) h += 1;

        const hp = h * 6;
        const x = 1 - Math.abs((hp % 2) - 1);
        let cr = 0, cg = 0, cb = 0;
        if (hp < 1) { cr = 1; cg = x; cb = 0; }
        else if (hp < 2) { cr = x; cg = 1; cb = 0; }
        else if (hp < 3) { cr = 0; cg = 1; cb = x; }
        else if (hp < 4) { cr = 0; cg = x; cb = 1; }
        else if (hp < 5) { cr = x; cg = 0; cb = 1; }
        else { cr = 1; cg = 0; cb = x; }

        outData[i] = Math.round(cr * 255);
        outData[i + 1] = Math.round(cg * 255);
        outData[i + 2] = Math.round(cb * 255);
      }
      outData[i + 3] = 255;
    }
  } else if (checkLayer === "solar") {
    for (let i = 0; i < data.length; i += 4) {
      const a = data[i + 3];
      if (a < 10) continue;
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      const y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
      const val = Math.round(127.5 * (1 + Math.sin((y * 6 * Math.PI) / 255)));
      outData[i] = val;
      outData[i + 1] = val;
      outData[i + 2] = val;
      outData[i + 3] = 255;
    }
  }

  ctx.putImageData(out, 0, 0);
}
