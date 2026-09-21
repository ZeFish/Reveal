import gardenThemes from "./garden-themes.generated.json";

export const DEFAULT_DARK_BG = "#15110D";
export const DEFAULT_ACCENT = "#D6202C";

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
 * }} StoryThemeState
 */

export const storyTheme = $state({
  darkBackground: /** @type {string | null} */ (null),
  darkAccent: /** @type {string | null} */ (null),
  fontHeader: /** @type {string | null} */ (null),
  fontText: /** @type {string | null} */ (null),
});

/**
 * @param {Partial<StoryThemeState>} tokens
 */
export function updateStoryTheme(tokens) {
  if (tokens.darkBackground !== undefined) storyTheme.darkBackground = tokens.darkBackground;
  if (tokens.darkAccent !== undefined) storyTheme.darkAccent = tokens.darkAccent;
  if (tokens.fontHeader !== undefined) storyTheme.fontHeader = tokens.fontHeader;
  if (tokens.fontText !== undefined) storyTheme.fontText = tokens.fontText;
}

export function getStoryThemeName() {
  const match = gardenThemes.themes.find(
    (t) =>
      t.darkBackground?.toLowerCase() === storyTheme.darkBackground?.toLowerCase() &&
      t.darkAccent?.toLowerCase() === storyTheme.darkAccent?.toLowerCase()
  );
  return match?.label ?? (storyTheme.darkBackground ? "Custom" : "Default (Garden)");
}
