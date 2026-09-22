import gardenThemes from "./garden-themes.generated.json";

export const DEFAULT_DARK_BG = "#15110D";
export const DEFAULT_ACCENT = "#D6202C";

// A folder theme only ever models its DARK tokens (darkBackground/
// darkAccent) — light is always the derived swap (contrastInk). Anything
// that paints a themed surface needs to know which half of that swap is
// live right now, so this is one shared, module-singleton source of truth
// (matchMedia only ever needs to be wired up once) instead of every
// consumer polling window.matchMedia itself, which is what left StoryView's
// own preview canvas stuck on darkBackground even after +page.svelte's
// chrome (.cull/.app) was fixed to respect it.
export const colorScheme = $state({ prefersDark: true });
if (typeof window !== "undefined" && window.matchMedia) {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  colorScheme.prefersDark = mq.matches;
  mq.addEventListener("change", (e) => { colorScheme.prefersDark = e.matches; });
}

/**
 * @param {string | null | undefined} hex
 * @returns {string}
 */
export function contrastInk(hex) {
  if (!hex) return "#F5F1EA";
  const s = hex.replace("#", "");
  if (s.length !== 6) return "#F5F1EA";
  const v = parseInt(s, 16);
  const r = ((v >> 16) & 0xff) / 255;
  const g = ((v >> 8) & 0xff) / 255;
  const b = (v & 0xff) / 255;
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return lum < 0.5 ? "#F5F1EA" : "#15110D";
}

/**
 * @param {string | null | undefined} font
 * @param {boolean} [isHeader=false]
 * @returns {string}
 */
export function getFontFamilyWithFallback(font, isHeader = false) {
  if (!font) return isHeader ? "var(--font-header, sans-serif)" : "var(--font-text, Georgia, serif)";
  const fLower = font.toLowerCase();
  if (fLower.includes("mono")) {
    return `"${font}", var(--font-monospace, monospace)`;
  }
  if (
    fLower.includes("serif") ||
    fLower.includes("fern") ||
    fLower.includes("baskerville") ||
    fLower.includes("bookerly") ||
    fLower.includes("jenson") ||
    fLower.includes("burns")
  ) {
    return `"${font}", Georgia, "Times New Roman", serif`;
  }
  return `"${font}", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`;
}

/**
 * @typedef {{
 *   darkBackground: string | null,
 *   darkAccent: string | null,
 *   fontHeader: string | null,
 *   fontText: string | null,
 *   fontRatio: string | null,
 * }} StoryThemeState
 */

export const storyTheme = $state({
  darkBackground: /** @type {string | null} */ (null),
  darkAccent: /** @type {string | null} */ (null),
  fontHeader: /** @type {string | null} */ (null),
  fontText: /** @type {string | null} */ (null),
  // The heading modular-scale multiplier (--font-ratio) — how much bigger
  // each heading level reads relative to the base size. Independent of the
  // curated theme presets (none of them set it), so picking a theme never
  // resets a fine-tuned ratio.
  fontRatio: /** @type {string | null} */ (null),
});

/**
 * @param {Partial<StoryThemeState>} tokens
 */
export function updateStoryTheme(tokens) {
  if (tokens.darkBackground !== undefined) storyTheme.darkBackground = tokens.darkBackground;
  if (tokens.darkAccent !== undefined) storyTheme.darkAccent = tokens.darkAccent;
  if (tokens.fontHeader !== undefined) storyTheme.fontHeader = tokens.fontHeader;
  if (tokens.fontText !== undefined) storyTheme.fontText = tokens.fontText;
  if (tokens.fontRatio !== undefined) storyTheme.fontRatio = tokens.fontRatio;
}

function matchingCuratedTheme() {
  return gardenThemes.themes.find(
    (t) =>
      t.darkBackground?.toLowerCase() === storyTheme.darkBackground?.toLowerCase() &&
      t.darkAccent?.toLowerCase() === storyTheme.darkAccent?.toLowerCase()
  );
}

export function getStoryThemeName() {
  const match = matchingCuratedTheme();
  return match?.label ?? (storyTheme.darkBackground ? "Custom" : "Default (Garden)");
}

/**
 * A saved folder theme only stores fontHeader/fontText NAMES (e.g.
 * "Forrest"), not which @font-face package actually declares them — that
 * mapping only exists on the curated theme entry itself (fontPackages).
 * Used to re-load a theme's fonts after story_load_theme reads a folder's
 * already-saved theme back from disk, where only the names come back.
 * @returns {string[]}
 */
export function currentThemeFontPackages() {
  return matchingCuratedTheme()?.fontPackages ?? [];
}
