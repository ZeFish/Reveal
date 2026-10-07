/**
 * A theme colour as 0-255 RGB, for the places that paint pixels (a canvas has no CSS
 * variables). Themes write their palette in any CSS colour syntax — hex, oklch(), even
 * `oklch(from … calc(h + 25))` — so the variable is resolved by the browser (an element
 * whose `color` is the variable), and the computed colour is read back through a canvas.
 *
 * @param {string} variable e.g. "--color-blue"
 * @param {[number, number, number]} fallback used where there is no DOM, or no such colour
 * @returns {[number, number, number]}
 */
export function themeRGB(variable, fallback) {
  if (typeof document === "undefined" || !document.body) return fallback;
  const probe = document.createElement("span");
  probe.style.color = `var(${variable})`;
  probe.style.display = "none";
  document.body.appendChild(probe);
  const resolved = getComputedStyle(probe).color;
  probe.remove();
  if (!resolved) return fallback;
  try {
    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;
    const ctx = canvas.getContext("2d");
    if (!ctx) return fallback;
    ctx.fillStyle = "#010203"; // a value the colour cannot be mistaken for if parsing fails
    ctx.fillStyle = resolved;
    ctx.fillRect(0, 0, 1, 1);
    const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
    return r === 1 && g === 2 && b === 3 ? fallback : [r, g, b];
  } catch {
    return fallback;
  }
}

/** The zone colours, from the theme: the same ones that name the zones in the panel. */
export function zoneColors() {
  return {
    shadows: themeRGB("--color-blue", [0, 130, 255]),
    midtones: themeRGB("--color-green", [30, 200, 70]),
    highlights: themeRGB("--color-red", [255, 40, 40]),
  };
}
