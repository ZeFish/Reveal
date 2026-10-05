/**
 * Tidy operations and plan formatting utilities.
 */

/**
 * Extracts directory name from path.
 * @param {string} path
 */
export function formatPathName(path) {
  if (!path) return "";
  return path.split("/").filter(Boolean).pop() ?? path;
}

/**
 * Formats a number with Canadian English thousands separator.
 * @param {number} count
 */
export function formatCount(count) {
  return (Number(count) || 0).toLocaleString("en-CA");
}

/**
 * Returns path relative to an archive base path.
 * @param {string} path
 * @param {string} base
 */
export function relativePathUnder(path, base) {
  if (!path) return "";
  if (!base) return path;
  return path.startsWith(`${base}/`) ? path.slice(base.length + 1) : path;
}

/**
 * Formats an array of [reason, count] pairs into a readable string.
 * @param {[string, number][]} reasons
 */
export function formatReasons(reasons) {
  if (!Array.isArray(reasons)) return "";
  return reasons.map(([r, c]) => `${formatCount(c)} ${r}`).join(" · ");
}

/**
 * Formats an array of [from_dir, count] pairs into a readable string.
 * @param {[string, number][]} fromDirs
 */
export function formatFromDirs(fromDirs) {
  if (!Array.isArray(fromDirs)) return "";
  return fromDirs.map(([f, c]) => `${formatPathName(f)} (${formatCount(c)})`).join(", ");
}

/**
 * Fetches the tidy plan from backend.
 * @param {string} dir
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 */
export async function fetchTidyPlan(dir, { invoke }) {
  if (!dir) return null;
  return await invoke("tidy_plan", { dir });
}
