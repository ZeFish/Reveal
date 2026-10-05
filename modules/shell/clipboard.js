/**
 * System clipboard helpers for photo and canvas copies.
 */

/**
 * @param {string} path
 * @param {(msg: string, ms?: number) => void} notify
 */
export function showCopiedMessage(path, notify) {
  const filename = path.split("/").pop();
  notify(`Image copied to the clipboard (${filename}) ✓`, 2500);
}

/**
 * Copy developed photo or raw preview directly to system clipboard.
 *
 * @param {string} path
 * @param {{
 *   currentMode?: string,
 *   photoPath?: string | null,
 *   developState?: { useCanvas?: boolean, canvasEl?: HTMLCanvasElement | null, recipe?: any },
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, ms?: number) => void,
 *   previewPx?: number,
 * }} context
 */
export async function copyImageToClipboard(path, context) {
  if (!path) return;
  const {
    currentMode = "cull",
    photoPath = null,
    developState = {},
    invoke,
    notify = () => {},
    previewPx = 2048,
  } = context;

  try {
    if (currentMode === "dev" && developState.useCanvas && developState.canvasEl) {
      const canvas = developState.canvasEl;
      const pngBlob = await new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
      if (pngBlob) {
        await navigator.clipboard.write([new ClipboardItem({ [pngBlob.type]: pngBlob })]);
        showCopiedMessage(path, notify);
      }
      return;
    }

    if (currentMode === "dev" && photoPath === path && developState.recipe) {
      await invoke("copy_developed_preview_to_clipboard", {
        path,
        recipe: developState.recipe,
        maxPx: previewPx,
      });
    } else {
      await invoke("copy_photo_preview_to_clipboard", { path });
    }
    showCopiedMessage(path, notify);
  } catch (err) {
    console.error("Could not copy image to clipboard:", err);
    notify(`Failed to copy the image: ${err}`, 3000);
  }
}
