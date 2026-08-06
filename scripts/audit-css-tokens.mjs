// CSS token audit for apps/reveal against the @stnd framework.
//
// DEFINED  = every custom property the compiled framework + reveal theme sets.
// USED     = every var(--x) reference in the app's own source.
//
// Reports, in order of severity:
//   A. used, no fallback, NOT defined  -> the whole CSS declaration is invalid
//                                         and silently dropped (the --accent bug)
//   B. used with a fallback, NOT defined -> renders, but off-theme forever
//   C. defined but never used            -> framework capability left on the table
import fs from "fs";
import path from "path";

// Resolve from this script's location so the audit works from any checkout.
const APP = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const FRAMEWORK_CSS = process.argv[2];

// ---- DEFINED ---------------------------------------------------------------
const css = fs.readFileSync(FRAMEWORK_CSS, "utf8");
const defined = new Set();
for (const m of css.matchAll(/(?:^|[;{\s"'(])(--[a-zA-Z0-9_-]+)\s*:/g)) defined.add(m[1]);

// The app itself may define tokens locally; those count as defined too.
const appDefined = new Set();

// ---- collect app sources ---------------------------------------------------
/** @param {string} dir @param {string[]} out */
function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (e.name === "node_modules" || e.name === "build" || e.name === ".svelte-kit") continue;
      walk(p, out);
    } else if (/\.(svelte|scss|css)$/.test(e.name)) out.push(p);
  }
  return out;
}
const files = [...walk(path.join(APP, "src")), ...walk(path.join(APP, "modules"))];

for (const f of files) {
  const src = fs.readFileSync(f, "utf8");
  for (const m of src.matchAll(/(?:^|[;{\s"'(])(--[a-zA-Z0-9_-]+)\s*:/g)) appDefined.add(m[1]);
}
const allDefined = new Set([...defined, ...appDefined]);

// ---- USED ------------------------------------------------------------------
/** @type {Map<string, {withFallback: boolean, sites: string[]}>} */
const used = new Map();
for (const f of files) {
  const src = fs.readFileSync(f, "utf8");
  const lines = src.split("\n");
  lines.forEach((line, i) => {
    for (const m of line.matchAll(/var\(\s*(--[a-zA-Z0-9_-]+)\s*(,)?/g)) {
      const name = m[1];
      const rel = path.relative(APP, f);
      const rec = used.get(name) ?? { withFallback: true, sites: [] };
      if (!m[2]) rec.withFallback = false; // at least one bare usage
      rec.sites.push(`${rel}:${i + 1}`);
      used.set(name, rec);
    }
  });
}

// ---- classify --------------------------------------------------------------
const brokenBare = [];
const fallbackOnly = [];
for (const [name, rec] of [...used].sort()) {
  if (allDefined.has(name)) continue;
  (rec.withFallback ? fallbackOnly : brokenBare).push([name, rec]);
}

const unused = [...defined].filter((d) => !used.has(d)).sort();

// ---- report ----------------------------------------------------------------
const line = (s = "") => console.log(s);
line("=".repeat(78));
line("CSS TOKEN AUDIT — apps/reveal vs @stnd framework + reveal theme");
line("=".repeat(78));
line(`framework/theme defines : ${defined.size} tokens`);
line(`app defines locally     : ${appDefined.size} tokens`);
line(`app references          : ${used.size} distinct tokens`);
line();

line("-".repeat(78));
line(`A. BROKEN — referenced with NO fallback, never defined  (${brokenBare.length})`);
line("   The entire CSS declaration is invalid and dropped. Renders as unstyled.");
line("-".repeat(78));
if (!brokenBare.length) line("   none ✓");
for (const [name, rec] of brokenBare) {
  line(`   ${name}   (${rec.sites.length} uses)`);
  [...new Set(rec.sites)].slice(0, 4).forEach((s) => line(`       ${s}`));
  if (new Set(rec.sites).size > 4) line(`       … +${new Set(rec.sites).size - 4} more`);
}
line();

line("-".repeat(78));
line(`B. FRAGILE — only ever used WITH a fallback, never defined  (${fallbackOnly.length})`);
line("   Renders via the fallback, so it silently ignores the theme forever.");
line("-".repeat(78));
if (!fallbackOnly.length) line("   none ✓");
for (const [name, rec] of fallbackOnly) {
  line(`   ${name}   (${rec.sites.length} uses)  e.g. ${[...new Set(rec.sites)][0]}`);
}
line();

line("-".repeat(78));
line(`C. UNUSED framework tokens  (${unused.length} of ${defined.size})`);
line("-".repeat(78));
const groups = {};
for (const t of unused) {
  const key = t.replace(/^--/, "").split("-")[0];
  (groups[key] ??= []).push(t);
}
for (const [g, list] of Object.entries(groups).sort((a, b) => b[1].length - a[1].length)) {
  line(`   ${g} (${list.length}): ${list.slice(0, 8).join(", ")}${list.length > 8 ? ", …" : ""}`);
}
line();
line(`Coverage: ${(((defined.size - unused.length) / defined.size) * 100).toFixed(1)}% of framework tokens referenced.`);
