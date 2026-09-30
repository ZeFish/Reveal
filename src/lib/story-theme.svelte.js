// A folder's look is one thing: the Garden theme its story note names in
// `theme:`. No per-folder colours, fonts or scale — pick a theme and the
// framework supplies all of it (see applyFolderTheme in app-theme.js).
import gardenThemes from "./garden-themes.generated.json";

/** The themes a folder can wear, for the picker: `{ id, label, darkBackground, darkAccent, … }`. */
export const THEMES = gardenThemes.themes;

/** The open folder's theme id (e.g. "forest"), or null for the app's own. */
export const storyTheme = $state({ id: /** @type {string | null} */ (null) });

/** @param {string | null | undefined} id */
export function setStoryTheme(id) {
  storyTheme.id = id ? String(id) : null;
}

/**
 * A note written before `theme:` existed carries the theme's colours instead.
 * Recognise a curated theme by them so it can be moved to `theme:`.
 * @param {string | null | undefined} darkBackground
 * @param {string | null | undefined} darkAccent
 * @returns {string | null}
 */
export function themeFromColors(darkBackground, darkAccent) {
  if (!darkBackground || !darkAccent) return null;
  const match = THEMES.find(
    (t) =>
      t.darkBackground?.toLowerCase() === darkBackground.toLowerCase() &&
      t.darkAccent?.toLowerCase() === darkAccent.toLowerCase(),
  );
  return match ? String(match.id) : null;
}
