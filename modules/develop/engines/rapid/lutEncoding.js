/**
 * The format the LUT stacks work in (src-tauri/crates/reveal-engine/src/encoding.rs).
 * The Rust side names the same four words.
 */
export const LUT_ENCODINGS = [
  { value: "display", label: "Display (sRGB)" },
  { value: "logc3", label: "LogC3 (ARRI)" },
  { value: "cineon", label: "Cineon" },
  { value: "linear", label: "Linear" },
];

/**
 * The encoding a recipe asks for. A recipe saved before the menu only has the LogC switch,
 * which reads as LogC3 — the same rule as `Recipe::encoding` in Rust.
 * @param {{ lut_encoding?: string, use_logc?: boolean } | null | undefined} recipe
 * @returns {string}
 */
export function lutEncodingOf(recipe) {
  const chosen = recipe?.lut_encoding;
  if (chosen && chosen !== "display") return chosen;
  return recipe?.use_logc ? "logc3" : "display";
}
