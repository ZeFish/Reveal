import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";

/**
 * Every slider in the app draws its filled rail from `--f` (app.scss's
 * input[type="range"] track gradient). A CSS rail cannot read its own value,
 * so an input that does not set `--f` shows the fill at 50% whatever its
 * value — the Photo Size slider at 75% (Francis, 2026-09-28), the crop
 * angle, the heading-scale slider. This walks the source and fails on any
 * range input that forgets.
 */
const ROOTS = ["src", "modules"].map((d) => path.resolve(__dirname, "../..", d));

/** @param {string} dir @returns {string[]} */
function svelteFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) return e.name === "node_modules" ? [] : svelteFiles(p);
    return e.name.endsWith(".svelte") ? [p] : [];
  });
}

describe("slider rails", () => {
  it("every range input sets its --f fill", () => {
    const missing = [];
    for (const file of ROOTS.flatMap(svelteFiles)) {
      const src = fs.readFileSync(file, "utf8");
      // Each <input ...> tag, attributes and all, up to its closing `>` —
      // one not preceded by `=`, so an `(e) => ...` handler does not end it.
      for (const m of src.matchAll(/<input\b[\s\S]*?(?<!=)>/g)) {
        const tag = m[0];
        if (tag.includes('type="range"') && !tag.includes("--f")) {
          const line = src.slice(0, m.index).split("\n").length;
          missing.push(`${path.relative(process.cwd(), file)}:${line}`);
        }
      }
    }
    expect(missing).toEqual([]);
  });
});
