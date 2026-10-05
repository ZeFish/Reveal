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

/**
 * Render visual check layer (clipping, false color, saturation, hue, solar, zone masks) onto a canvas context.
 *
 * @param {ImageData} sourceData
 * @param {string} checkLayer
 * @param {CanvasRenderingContext2D} ctx
 * @param {number} width
 * @param {number} height
 */
export function renderCheckLayer(sourceData, checkLayer, ctx, width, height) {
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
        if (isShadows) {
          outData[i] = 0;
          outData[i + 1] = 130;
          outData[i + 2] = 255;
        } else if (isHighlights) {
          outData[i] = 255;
          outData[i + 1] = 40;
          outData[i + 2] = 40;
        } else {
          outData[i] = 30;
          outData[i + 1] = 200;
          outData[i + 2] = 70;
        }
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
        outData[i] = 255;
        outData[i + 1] = 30;
        outData[i + 2] = 30;
        outData[i + 3] = 230;
      } else if (r <= 3 && g <= 3 && b <= 3) {
        outData[i] = 0;
        outData[i + 1] = 120;
        outData[i + 2] = 255;
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

      if (y < 6) {
        outData[i] = 140; outData[i + 1] = 20; outData[i + 2] = 180;
      } else if (y < 26) {
        outData[i] = 15; outData[i + 1] = 60; outData[i + 2] = 220;
      } else if (y < 56) {
        outData[i] = 0; outData[i + 1] = 175; outData[i + 2] = 210;
      } else if (y < 96) {
        outData[i] = 65; outData[i + 1] = 65; outData[i + 2] = 65;
      } else if (y < 122) {
        outData[i] = 10; outData[i + 1] = 210; outData[i + 2] = 45;
      } else if (y < 148) {
        outData[i] = 255; outData[i + 1] = 130; outData[i + 2] = 165;
      } else if (y < 196) {
        outData[i] = 140; outData[i + 1] = 140; outData[i + 2] = 140;
      } else if (y < 232) {
        outData[i] = 255; outData[i + 1] = 225; outData[i + 2] = 0;
      } else if (y < 250) {
        outData[i] = 255; outData[i + 1] = 115; outData[i + 2] = 0;
      } else {
        outData[i] = 255; outData[i + 1] = 20; outData[i + 2] = 20;
      }
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
        outData[i] = 255; outData[i + 1] = 0; outData[i + 2] = 170;
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
