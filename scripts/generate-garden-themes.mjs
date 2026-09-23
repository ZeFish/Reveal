// Generates src/lib/garden-themes.generated.json from the Standard theme
// package (packages/themes/*/tokens.yaml) — the same source of truth the
// Swift/VS Code theme adapters read, so the story composer's theme dropdown
// can't drift from the real named themes the way a hand-copied list would.
//
// Re-run manually after editing any packages/themes/*/tokens.yaml:
//   pnpm --filter reveal generate:garden-themes
// (./install.sh — `pnpm build` — runs it on every install.)
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import yaml from "js-yaml";
import { oklchToRgb, hslToRgb, toHex, mix } from "../../../packages/themes/_scripts/utils/color.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const themesDir = path.resolve(__dirname, "../../../packages/themes");
const outPath = path.resolve(__dirname, "../src/lib/garden-themes.generated.json");

const HEX6_RE = /^#[0-9a-fA-F]{6}$/;
const HEX3_RE = /^#([0-9a-fA-F])([0-9a-fA-F])([0-9a-fA-F])$/;

/** @param {string} hex */
function normalizeHex(hex) {
  if (HEX6_RE.test(hex)) return hex;
  const m = hex.match(HEX3_RE);
  return m ? `#${m[1]}${m[1]}${m[2]}${m[2]}${m[3]}${m[3]}` : null;
}

// A token value may be a full CSS font-family stack (multiple comma-separated,
// individually-quoted names). Reveal's `story_set_theme` stores a bare single
// family name (see src-tauri/src/story.rs — `font_header: Some("Sohne"...)`,
// no quotes, no fallback chain), matching the existing curated FONTS list in
// StoryThemePanel.svelte — so only the first family name is kept.
/** @param {unknown} v */
function firstFontFamily(v) {
  if (typeof v !== "string" || !v) return null;
  const first = v.split(",")[0].trim().replace(/^["']|["']$/g, "").trim();
  return first || null;
}

// Some themes never define their own magenta swatch and lean on purple/pink
// instead (same convention build.js's own `magenta: parseColorValue(schemeColor("purple", scheme)
// ?? schemeColor("pink", scheme), ...)` step already encodes for its "magenta"
// output slot — mirrored here for the general var(--color-magenta) case too).
const COLOR_ALIASES = { magenta: ["purple", "pink"] };

// Several themes explicitly reuse their light-side swatch for dark ("Reveal's
// accent is exactly this — same in both schemes", per build.js) by writing
// e.g. `color-dark-background: var(--color-light-background)` by hand. Bake
// that same reuse in as a last-resort fallback for bare colour names (accent,
// magenta, etc.) that never got an explicit dark-side definition at all.
/**
 * @param {Record<string, unknown>} tokens
 * @param {string} name
 * @param {string} scheme
 * @returns {unknown}
 */
function lookupColorName(tokens, name, scheme) {
  const direct =
    tokens[`color-${scheme}-${name}`] ?? tokens[`color-${name}`] ?? tokens[`color-light-${name}`];
  if (direct) return direct;
  for (const alias of COLOR_ALIASES[name] ?? []) {
    const v = lookupColorName(tokens, alias, scheme);
    if (v) return v;
  }
  return undefined;
}

/**
 * `color-mix(in <space>, A [P%], B [P%])` → resolve A/B recursively, then
 * blend at the given weights (a missing percentage is the complement of the
 * other, or 50/50 if both are missing — the CSS color-mix() defaults).
 * @param {string} value
 * @param {Record<string, unknown>} tokens
 * @param {string} scheme
 * @returns {string | null}
 */
function resolveColorMix(value, tokens, scheme) {
  const inner = value.match(/^color-mix\(\s*(.*)\s*\)$/s)?.[1];
  if (!inner) return null;
  const parts = inner.split(",").map((s) => s.trim());
  if (parts.length !== 3) return null; // "in <space>", component 1, component 2

  /** @param {string} raw */
  const parseComponent = (raw) => {
    const m = raw.match(/^(.*?)(?:\s+([\d.]+)%)?$/);
    return { expr: m?.[1]?.trim() ?? raw, pct: m?.[2] !== undefined ? Number(m[2]) : null };
  };
  const c1 = parseComponent(parts[1]);
  const c2 = parseComponent(parts[2]);
  let w1 = c1.pct;
  let w2 = c2.pct;
  if (w1 == null && w2 == null) {
    w1 = 50;
    w2 = 50;
  } else if (w1 == null) w1 = 100 - /** @type {number} */ (w2);
  else if (w2 == null) w2 = 100 - w1;

  const hex1 = resolveColor(c1.expr, tokens, scheme);
  const hex2 = resolveColor(c2.expr, tokens, scheme);
  if (!hex1 || !hex2) return null;
  return mix(hex1, hex2, w2 / (w1 + w2));
}

// Mirrors the colour resolution in packages/themes/_scripts/build.js
// (var(--...) indirection, oklch()/hsl()/color-mix() -> hex), extended to
// also handle the oklch() percentage-lightness form and color-mix() (neither
// of which build.js itself resolves). Kept as a standalone copy rather than
// importing build.js, since that module also runs the full multi-adapter
// generation pipeline as a side effect of import. Anything that still doesn't
// resolve to a plain #rrggbb hex is rejected by the caller — <input
// type="color"> only accepts hex.
/**
 * @param {unknown} value
 * @param {Record<string, unknown>} tokens
 * @param {string} scheme
 * @returns {string | null}
 */
function resolveColor(value, tokens, scheme) {
  if (typeof value !== "string" || !value) return null;
  if (value.startsWith("var(--")) {
    const varName = value.match(/var\(--(.*?)\)/)?.[1];
    if (!varName) return null;
    // Exact key first — handles explicit cross-scheme refs like
    // `var(--color-light-background)`, which isn't a bare colour name.
    let resolved = tokens[varName];
    if (!resolved && varName.startsWith("color-")) {
      resolved = lookupColorName(tokens, varName.replace("color-", ""), scheme);
    }
    return resolveColor(resolved, tokens, scheme);
  }
  if (value.startsWith("color-mix(")) return resolveColorMix(value, tokens, scheme);
  if (value.startsWith("oklch(")) {
    const raw = value
      .match(/oklch\(\s*(.*?)\s*\)/)?.[1]
      ?.split(/\s+/)
      .filter(Boolean);
    if (raw?.length !== 3) return null;
    const L = raw[0].endsWith("%") ? parseFloat(raw[0]) / 100 : Number(raw[0]);
    const C = Number(raw[1]);
    const H = Number(raw[2]);
    if ([L, C, H].some(Number.isNaN)) return null;
    return toHex(oklchToRgb(L, C, H));
  }
  if (value.startsWith("hsl(")) {
    const parts = value
      .match(/hsl\((.*?)\)/)?.[1]
      ?.split(",")
      .map((s) => parseFloat(s));
    if (parts?.length === 3) return toHex(hslToRgb(parts[0], parts[1], parts[2]));
    return null;
  }
  return normalizeHex(value.replace(/"/g, "").trim());
}

/**
 * @param {Record<string, unknown>} tokens
 * @param {string} name
 * @param {string} scheme
 */
function schemeColor(tokens, name, scheme) {
  return lookupColorName(tokens, name, scheme);
}

// Which font package declares which family — read from each package's own
// @font-face rules, the only place a family name is actually defined. Keys
// are lowercased: CSS matches family names case-insensitively.
const fontsDir = path.resolve(__dirname, "../../../packages/fonts");
/** @type {Map<string, string>} family (lowercase) → package slug */
const familyToPackage = new Map();
const fontPackageSlugs = new Set();
for (const slug of fs.readdirSync(fontsDir)) {
  const css = path.join(fontsDir, slug, `${slug}.css`);
  if (!fs.existsSync(css)) continue;
  fontPackageSlugs.add(slug);
  for (const m of fs.readFileSync(css, "utf8").matchAll(/font-family:\s*["']?([^"';]+?)["']?\s*;/g)) {
    familyToPackage.set(m[1].toLowerCase(), slug);
  }
}

// Families a browser or macOS supplies on its own — no package expected.
const BUILT_IN = new Set(
  ("serif sans-serif monospace system-ui cursive fantasy ui-monospace ui-sans-serif ui-serif " +
    "-apple-system blinkmacsystemfont inherit initial")
    .split(" ")
    .concat(["helvetica", "helvetica neue", "arial", "georgia", "times", "times new roman", "menlo",
      "monaco", "courier", "courier new", "consolas", "sf mono", "new york", "avenir", "avenir next",
      "futura", "palatino", "optima", "gill sans", "baskerville", "didot", "charter", "segoe ui",
      "roboto", "iowan old style", "hoefler text", "american typewriter", "verdana", "impact",
      "arial narrow", "sfmono-regular"]),
);

/**
 * Every family named in a font stack: `"Fern", "Graveur Variable", Bookerly`
 * → ["Fern", "Graveur Variable", "Bookerly"]. var(...) references are skipped.
 * @param {string} stack
 */
function families(stack) {
  return stack
    .split(",")
    .map((f) => f.trim().replace(/^["']|["']$/g, "").trim())
    .filter((f) => f && !f.startsWith("var("));
}

/** @type {string[]} */
const fontWarnings = [];

const entries = [];
for (const dirName of fs.readdirSync(themesDir).sort()) {
  const themeDir = path.join(themesDir, dirName);
  const tokensPath = path.join(themeDir, "tokens.yaml");
  if (!fs.statSync(themeDir).isDirectory() || !fs.existsSync(tokensPath)) continue;

  const raw = /** @type {any} */ (yaml.load(fs.readFileSync(tokensPath, "utf8")));
  if (!raw?.tokens) continue;

  const tokens = raw.tokens;
  const meta = raw.meta ?? {};
  const id = meta.id || dirName;
  const label = meta.label || id.charAt(0).toUpperCase() + id.slice(1);

  const darkBackground = resolveColor(schemeColor(tokens, "background", "dark"), tokens, "dark");
  const darkAccent = resolveColor(schemeColor(tokens, "accent", "dark"), tokens, "dark");
  if (!darkBackground || !darkAccent) {
    console.warn(`Skipping "${id}": dark background/accent unresolvable or not a plain hex color`);
    continue;
  }

  // The font packages to load. Hand-maintained meta.fonts drifted: on
  // 2026-09-23, 12 of 28 themes used a family whose package was missing from
  // it or misnamed ("IBM Plex", "Kalice"), so the app silently fell back.
  // Now DERIVED from what the theme actually uses — every family in every
  // font token of its .scss (hand-written overrides included, which is what
  // the app applies) and of tokens.yaml — resolved through the packages' own
  // @font-face names. meta.fonts still counts, normalised to a slug, for
  // packages a theme wants loaded that no token names.
  const themeScss = path.join(themeDir, `${dirName}.scss`);
  const stacks = [
    ...Object.entries(tokens)
      .filter(([k]) => /^font-(text|header|interface|monospace)$/.test(k))
      .map(([, v]) => String(v)),
    ...(fs.existsSync(themeScss)
      ? [...fs.readFileSync(themeScss, "utf8").matchAll(/--font-(?:text|header|interface|monospace):\s*([^;]+);/g)].map((m) => m[1])
      : []),
  ];
  const packages = new Set();
  for (const f of Array.isArray(meta.fonts) ? meta.fonts : []) {
    const slug = String(f).split("/").pop()?.toLowerCase().replace(/\s+/g, "-");
    if (slug && fontPackageSlugs.has(slug)) packages.add(slug);
    else fontWarnings.push(`${id}: meta.fonts names "${f}", which is no package in packages/fonts`);
  }
  const unresolved = new Set();
  for (const stack of stacks) {
    for (const family of families(stack)) {
      const key = family.toLowerCase();
      const pkg = familyToPackage.get(key);
      if (pkg) packages.add(pkg);
      else if (!BUILT_IN.has(key)) unresolved.add(family);
    }
  }
  for (const family of unresolved) {
    fontWarnings.push(`${id}: "${family}" is declared by no font package — falls back to the next font in its stack`);
  }
  const fontPackages = [...packages].sort();

  entries.push({
    id,
    label,
    darkBackground,
    darkAccent,
    fontHeader: firstFontFamily(tokens["font-header"]),
    fontText: firstFontFamily(tokens["font-text"]),
    fontPackages,
  });
}

entries.sort((a, b) => a.label.localeCompare(b.label));

fs.mkdirSync(path.dirname(outPath), { recursive: true });
fs.writeFileSync(
  outPath,
  JSON.stringify(
    {
      _comment:
        "AUTO-GENERATED by apps/reveal/scripts/generate-garden-themes.mjs — do not edit. Source: packages/themes/*/tokens.yaml",
      themes: entries,
    },
    null,
    2,
  ) + "\n",
  "utf8",
);

console.log(`Wrote ${entries.length} garden themes to ${path.relative(process.cwd(), outPath)}`);
if (fontWarnings.length) {
  console.warn(`\n⚠️  ${fontWarnings.length} font reference(s) nothing can load:`);
  for (const w of fontWarnings) console.warn(`   ${w}`);
}
