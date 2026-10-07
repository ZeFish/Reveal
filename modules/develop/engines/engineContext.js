/**
 * Shared utility functions for digital darkroom engines:
 * - Recipe and zone target resolution
 * - Value conversion and display formatting
 * - Slider neutral points and scaling
 */

export const INVERTED_CONTROLS = new Set(["print_exposure_ev"]);

/** Stored as 0..1 in the recipe, shown as a percentage. */
export const PERCENT_CONTROLS = new Set(["film_prep"]);

/**
 * Like {@link getActiveTarget} but never writes: a zone that does not exist yet
 * reads as empty. Use it wherever a value is only displayed — rendering must not
 * mutate state (Svelte throws `state_unsafe_mutation`).
 * @param {any} recipe
 * @param {string} [activeZone]
 * @returns {any}
 */
export function peekActiveTarget(recipe, activeZone = "global") {
  if (!recipe || !activeZone || activeZone === "global") return recipe;
  return recipe[`zone_${activeZone}`] ?? EMPTY_TARGET;
}

const EMPTY_TARGET = Object.freeze({});

/**
 * Resolves the active target inside a recipe (global or zone_shadows / zone_midtones / zone_highlights),
 * creating the zone if it is missing. Only call it from an event handler.
 * @param {any} recipe
 * @param {string} [activeZone]
 * @returns {any}
 */
export function getActiveTarget(recipe, activeZone = "global") {
  if (!recipe || !activeZone || activeZone === "global") return recipe;
  const key = `zone_${activeZone}`;
  if (!recipe[key]) recipe[key] = {};
  return recipe[key];
}

/**
 * @param {string} id
 * @param {number | undefined} v
 * @returns {number}
 */
export function toDisplay(id, v) {
  const val = v ?? 0;
  if (PERCENT_CONTROLS.has(id)) return val * 100;
  return INVERTED_CONTROLS.has(id) ? -val : val;
}

/**
 * @param {string} id
 * @param {number} v
 * @returns {number}
 */
export function fromDisplay(id, v) {
  if (PERCENT_CONTROLS.has(id)) return v / 100;
  return INVERTED_CONTROLS.has(id) ? -v : v;
}

/**
 * Returns the neutral point for a slider control.
 * In zone modes, all sliders have a neutral of 0.
 * In global mode, the neutral point comes from the engine's default recipe.
 *
 * @param {any} defaults
 * @param {string} activeZone
 * @param {string} id
 * @param {number | null | undefined} [index]
 * @returns {number | undefined}
 */
export function getNeutral(defaults, activeZone, id, index) {
  if (activeZone && activeZone !== "global") return 0;
  const v = index == null ? defaults?.[id] : defaults?.[id]?.[index];
  return typeof v === "number" ? toDisplay(id, v) : undefined;
}

/**
 * The Temp slider runs -100..+100 around 5500 K; the number shown is the kelvin it means.
 * @param {number} v
 * @returns {number}
 */
export function temperatureToKelvin(v) {
  return v <= 0 ? 5500 + v * 35 : 5500 + v * 45;
}

/**
 * The slider value for a typed kelvin: the inverse of {@link temperatureToKelvin}.
 * @param {number} kelvin
 * @returns {number}
 */
export function kelvinToTemperature(kelvin) {
  return kelvin <= 5500 ? (kelvin - 5500) / 35 : (kelvin - 5500) / 45;
}

/**
 * Formats a numeric slider value for display.
 * @param {string} id
 * @param {number | undefined | null} v
 * @returns {string}
 */
export function formatVal(id, v) {
  if (v === undefined || v === null) return "0";
  if (id === "temperature") {
    return `${Math.round(temperatureToKelvin(v))} K`;
  }
  if (id === "exposure_ev" || id === "vignette_amount") {
    return (v > 0 ? "+" : "") + v.toFixed(2);
  }
  if (
    id === "contrast" ||
    id === "saturation" ||
    id === "density_gamma" ||
    id === "zone_shadows_exposure" ||
    id === "zone_midtones_exposure" ||
    id === "zone_highlights_exposure" ||
    id === "zone_shadows_contrast" ||
    id === "zone_midtones_contrast" ||
    id === "zone_highlights_contrast"
  ) {
    return (v > 0 ? "+" : "") + v.toFixed(2);
  }
  if (
    id === "vignette_midpoint" ||
    id === "vignette_roundness" ||
    id === "vignette_feather" ||
    id === "grain_amount" ||
    id === "grain_roughness" ||
    id === "highlight_desat" ||
    id === "glare_percent" ||
    id === "glare_roughness" ||
    id === "glare_blur" ||
    id === "preflash_exposure" ||
    id === "dir_couplers_amount" ||
    id === "dir_couplers_tail_weight"
  ) {
    return v.toFixed(2);
  }
  if (id === "development_time_min") {
    return v <= 0 ? "Auto" : `${v.toFixed(1)} min`;
  }
  if (id === "film_format_mm") {
    return `${Math.round(v)} mm`;
  }
  if (id === "film_prep") {
    return v <= 0 ? "Off" : `${Math.round(v)}%`;
  }
  return (v > 0 ? "+" : "") + Math.round(v);
}
