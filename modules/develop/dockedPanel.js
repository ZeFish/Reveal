/**
 * Whether the Develop panel is docked beside the photo.
 *
 * Space opens a photo in Develop as a quick look — the developed picture, nothing around it —
 * and `D` brings the panel in. The panel's own open/closed state is remembered between sessions
 * and was never consulted by the quick look, so a panel left open reappeared on every Space.
 * A quick look never shows it.
 *
 * @param {{ hasRecipe: boolean, devPanel: boolean, detached: boolean, isTauri: boolean, spaceLook: boolean }} o
 */
export function showsDockedPanel({ hasRecipe, devPanel, detached, isTauri, spaceLook }) {
  return hasRecipe && devPanel && !spaceLook && (!isTauri || !detached);
}
