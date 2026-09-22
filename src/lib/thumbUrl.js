/**
 * The one definition of a `reveal://thumb` URL.
 *
 * It lived in two places with two different shapes — the grid omitted `size`
 * and relied on the Rust default, the page spelled it out — so the very image
 * a cell had just painted could never be reused when that photo was opened:
 * different string, different webview cache entry, full refetch. Opening a
 * photo showed its empty mat while bytes already on screen were fetched
 * again (Francis, 2026-09-22).
 *
 * @param {string} path
 * @param {number} [version] the preview's mtime — busts both caches on edit
 * @param {number} [size] long edge in px; must match the grid's to share
 */
export function thumbUrl(path, version = 0, size = 768) {
  return `reveal://thumb?p=${encodeURIComponent(path)}&v=${version}&size=${size}`;
}
