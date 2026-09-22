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
// "reveal" is excluded here — +layout.svelte imports it statically (it's
// the always-on default, needed before any preference load resolves), so
// including it in the lazy set would just bundle a second, unused copy.
const themeModules = import.meta.glob([
  "../../../../packages/themes/*/*.scss",
  "!../../../../packages/themes/__archives__/**",
  "!../../../../packages/themes/reveal/reveal.scss",
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
export const DEFAULT_THEME = "reveal";

// "reveal" is imported statically by +layout.svelte already — never re-fetch it.
const loadedThemes = new Set([DEFAULT_THEME]);

// A theme's own tokens.yaml (meta.fonts) names the font packages it wants —
// see generate-garden-themes.mjs, which copies that array straight into
// garden-themes.generated.json as `fontPackages`. Loading fonts by package
// reference (not by the font-header/font-text DISPLAY name) sidesteps a real
// mismatch: Chalky's font-text is "Jimmy Serif Pro" but the package is
// packages/fonts/jimmy/, whose own @font-face declares yet a third name
// ("Jimmy Sans Pro") — none of the three strings match by slugifying.
import gardenThemesData from "./garden-themes.generated.json";
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

// Every component picks its own hover-transition timing (120ms here, 150ms
// there, some with no transition at all) — fine for a hover, but a color
// SCHEME change (theme pick, or macOS system light/dark) recolors dozens of
// them at once, and those mismatched durations turn one swap into a visible
// wave instead of a single fade. This forces one shared, synchronized
// duration (--duration-standard, 180ms in the app context — see app.scss)
// for the swap, then gets out of the way so hover transitions go back to
// their own timing. The 260ms timeout is that plus a buffer, not a duration
// of its own — keep it above whatever --duration-standard resolves to.
let transitionSyncTimer = /** @type {ReturnType<typeof setTimeout> | undefined} */ (undefined);
export function syncThemeTransition() {
  const root = document.documentElement;
  root.classList.add("theme-transitioning");
  clearTimeout(transitionSyncTimer);
  transitionSyncTimer = setTimeout(() => root.classList.remove("theme-transitioning"), 260);
}

/** @param {string} theme */
async function ensureThemeCssLoaded(theme) {
  if (!THEME_LOADERS[theme] || loadedThemes.has(theme)) return;
  await THEME_LOADERS[theme]();
  loadedThemes.add(theme);
}

/**
 * Switches the app's active theme, loading its token file on first use.
 * @param {string | null | undefined} id
 */
export async function applyTheme(id) {
  const theme = id && THEME_LOADERS[id] ? id : DEFAULT_THEME;
  await ensureThemeCssLoaded(theme);
  await loadThemeFonts(theme);
  syncThemeTransition();
  document.documentElement.dataset.theme = theme;
}

// A theme's [data-theme="<id>"] rule sets far more than the handful of
// color/font tokens the folder-theme system (garden-themes.generated.json)
// hand-picks — radius, font-weights, letter-spacing, all derived the same
// way colors are. That derivation only ever resolves at :root in this
// framework by design ($stnd-theme-scope, packages/styles), so a folder
// theme applied as a NESTED override (this app's own chrome stays on its
// own separate theme) never picks those up: "the corner radius of forest
// is not following". Reconfiguring $stnd-theme-scope package-wide would
// duplicate every derived token under a selector for every consumer of
// this framework, not just this one preview — too broad a change for what
// this needs. Instead: load the theme's real CSS, let the browser resolve
// it on a real (offscreen) element carrying that data-theme, read back
// whichever custom properties are asked for, and hand back plain values a
// caller can apply as its own inline overrides. Contained entirely to
// Reveal's own code, no shared-package risk.
/**
 * @param {string} id
 * @param {string[]} props e.g. ["--radius", "--radius-sm", "--radius-lg"]
 * @returns {Promise<Record<string, string>>}
 */
export async function measureThemeTokens(id, props) {
  if (!THEME_LOADERS[id]) return {};
  await ensureThemeCssLoaded(id);
  const probe = document.createElement("div");
  probe.dataset.theme = id;
  probe.style.cssText = "position:absolute;visibility:hidden;pointer-events:none;width:0;height:0;";
  document.body.appendChild(probe);
  const computed = getComputedStyle(probe);
  /** @type {Record<string, string>} */
  const out = {};
  for (const p of props) {
    const v = computed.getPropertyValue(p).trim();
    if (v) out[p] = v;
  }
  probe.remove();
  return out;
}
