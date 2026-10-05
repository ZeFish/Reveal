// Lets Reveal's own chrome wear any Standard theme, not just its dedicated
// "reveal" identity. Each theme's raw color/font seeds live in
// packages/themes/<id>/<id>.scss, scoped under [data-theme="<id>"] —
// packages/styles/_standard-02-color.scss derives every other token
// (surfaces, on-accent, etc.) generically from those seeds, so loading the
// file and flipping the attribute is the whole mechanism.
//
// Themes are lazy-loaded (import.meta.glob, one dynamic import per pick)
// rather than all bundled upfront — the 50+ theme files are ~1MB combined,
// most of which a given user will never look at.
// "macos" is excluded here — +layout.svelte imports it statically (it's
// the always-on default, needed before any preference load resolves), so
// including it in the lazy set would just bundle a second, unused copy.
const themeModules = import.meta.glob([
  "../../../../packages/themes/*/*.scss",
  "!../../../../packages/themes/__archives__/**",
  "!../../../../packages/themes/macos/macos.scss",
]);

// Keep only <id>/<id>.scss entries. A theme's own generated/ subfolder and
// __archives__'s renamed files never match their own dirname, so they fall
// out here with no hardcoded exclude list to keep in sync.
/** @type {Record<string, () => Promise<unknown>>} */
const THEME_LOADERS = {};
for (const [path, loader] of Object.entries(themeModules)) {
  const m = path.match(/\/([^/]+)\/\1\.scss$/);
  if (m) THEME_LOADERS[m[1]] = loader;
}

export const THEME_IDS = Object.keys(THEME_LOADERS);
export const DEFAULT_THEME = "macos";

// "macos" is imported statically by +layout.svelte already — never re-fetch it.
const loadedThemes = new Set([DEFAULT_THEME]);

// A theme's own tokens.yaml (meta.fonts) names the font packages it wants —
// see generate-garden-themes.mjs, which copies that array straight into
// garden-themes.generated.json as `fontPackages`. Loading fonts by package
// reference (not by the font-header/font-text DISPLAY name) sidesteps a real
// mismatch: Chalky's font-text is "Jimmy Serif Pro" but the package is
// packages/fonts/jimmy/, whose own @font-face declares yet a third name
// ("Jimmy Sans Pro") — none of the three strings match by slugifying.
import gardenThemesData from "./garden-themes.generated.json";
export const GARDEN_THEMES = gardenThemesData.themes;
/** @type {Record<string, string[]>} */
const THEME_FONT_PACKAGES = Object.fromEntries(
  gardenThemesData.themes.map((t) => [String(t.id), t.fontPackages ?? []]),
);

const fontModules = import.meta.glob(["../../../../packages/fonts/*/*.css"]);
/** @type {Record<string, () => Promise<unknown>>} */
const FONT_LOADERS = {};
for (const [path, loader] of Object.entries(fontModules)) {
  const m = path.match(/\/([^/]+)\/\1\.css$/);
  if (m) FONT_LOADERS[m[1]] = loader;
}
// "reveal"'s fonts are imported statically by +layout.svelte already (see
// its own comment — fonts aren't reachable through package export maps).
const loadedFontPackages = new Set(THEME_FONT_PACKAGES[DEFAULT_THEME] ?? []);

/** @param {string[]} packages */
export async function loadFontPackages(packages) {
  await Promise.all(
    packages
      .filter((pkg) => !loadedFontPackages.has(pkg) && FONT_LOADERS[pkg])
      .map((pkg) => {
        loadedFontPackages.add(pkg);
        return FONT_LOADERS[pkg]();
      }),
  );
}

/** @param {string} theme */
async function loadThemeFonts(theme) {
  await loadFontPackages(THEME_FONT_PACKAGES[theme] ?? []);
}

/** @param {string} theme */
async function ensureThemeCssLoaded(theme) {
  if (!THEME_LOADERS[theme] || loadedThemes.has(theme)) return;
  await THEME_LOADERS[theme]();
  loadedThemes.add(theme);
}

// <html>'s data-theme is the app's theme, unless the open folder carries a
// curated one. Both set a wish; the latest wins, and the attribute is always
// painted from the pair so neither clobbers the other.
let appTheme = DEFAULT_THEME;
/** @type {string | null} */
let folderTheme = null;
let folderRequest = 0;

function paintTheme() {
  document.documentElement.dataset.theme = folderTheme ?? appTheme;
}

/**
 * Switches the app's active theme, loading its token file on first use.
 * @param {string | null | undefined} id
 */
export async function applyTheme(id) {
  const theme = id && THEME_LOADERS[id] ? id : DEFAULT_THEME;
  await ensureThemeCssLoaded(theme);
  await loadThemeFonts(theme);
  appTheme = theme;
  paintTheme();
}

/**
 * The open folder's curated theme, worn over the app's until the folder is
 * left (null).
 * @param {string | null | undefined} id
 */
export async function applyFolderTheme(id) {
  const request = ++folderRequest;
  const theme = id && (id === DEFAULT_THEME || THEME_LOADERS[id]) ? id : null;
  if (theme) {
    await ensureThemeCssLoaded(theme);
    await loadThemeFonts(theme);
    if (request !== folderRequest) return;
  }
  folderTheme = theme;
  paintTheme();
}

// ---- interface text size ---------------------------------------------------
// One knob. app.scss (the framework's) fixes --font-text-size for a window;
// the type scale, the spacing rhythm (1rlh) and every control derive from it,
// so changing it scales the whole interface together. The preference is the
// size in px; the default leaves the framework's own value in place.
export const DEFAULT_TEXT_SIZE = 13;
export const TEXT_SIZES = [
  { px: 12, label: "Small" },
  { px: 13, label: "Default" },
  { px: 14, label: "Large" },
  { px: 15, label: "Larger" },
];

/** @param {number | null | undefined} px */
export function applyTextSize(px) {
  const style = document.documentElement.style;
  const size = TEXT_SIZES.some((t) => t.px === px) ? px : DEFAULT_TEXT_SIZE;
  if (size === DEFAULT_TEXT_SIZE) style.removeProperty("--font-text-size");
  else style.setProperty("--font-text-size", `${size}px`);
}
