/**
 * External editors manager.
 *
 * Discovers installed photo editing applications and handles opening
 * frames in them via Tauri commands.
 */

/**
 * @param {{
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, duration?: number) => void,
 *   isTauri?: boolean,
 *   getCurrentPhotoPath?: () => string | null | undefined,
 * }} deps
 */
export function createExternalEditorsController({
  invoke = async () => {},
  notify = () => {},
  isTauri = false,
  getCurrentPhotoPath = () => null,
}) {
  /** @type {[string, string][]} */
  let installedEditors = $state([]);

  async function loadExternalEditors() {
    if (isTauri) {
      try {
        installedEditors = await invoke("list_external_editors");
      } catch {
        installedEditors = [];
      }
    }
  }

  /**
   * Open the currently targeted frame in an external application.
   * @param {string} appPath
   */
  async function openInEditor(appPath) {
    const filePath = getCurrentPhotoPath();
    if (!appPath || !filePath) return;
    try {
      await invoke("open_in_editor", { filePath, appPath });
      notify("Opened successfully ✓", 2000);
    } catch (e) {
      notify(`Error: ${e}`, 5000);
    }
  }

  return {
    get installedEditors() {
      return installedEditors;
    },
    set installedEditors(v) {
      installedEditors = v;
    },
    loadExternalEditors,
    openInEditor,
  };
}
