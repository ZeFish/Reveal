import { session, Destination } from "@modules/core";
import { invoke } from "@tauri-apps/api/core";

export const exportState = $state({
  edge: 2048,
  border: false,
  folder: "",
  activeDestinationId: "folder",
});

export const exportDestinations = {
  get all() {
    return [
      Destination.localFolder(exportState.folder, {
        longEdge: exportState.edge,
        border: exportState.border,
      }),
      Destination.obsidian({
        longEdge: exportState.edge,
        border: exportState.border,
      }),
      Destination.garden(),
    ];
  },
  get active() {
    return this.all.find((d) => d.id === exportState.activeDestinationId || d.kind === exportState.activeDestinationId) ?? this.all[0];
  },
  /** @param {string} id */
  setActive(id) {
    exportState.activeDestinationId = id;
  },
};

export function saveExportPrefs() {
  session.setExportPrefs({
    edge: exportState.edge,
    border: exportState.border,
    folder: exportState.folder,
  });
}

/**
 * The Swift "DOSSIER" picker — chosen once, remembered; "" = the Desktop.
 * @param {() => void} [onFolderChanged]
 */
export async function chooseExportFolder(onFolderChanged) {
  const dest = await invoke("pick_folder");
  if (dest && typeof dest === "string") {
    exportState.folder = dest;
    saveExportPrefs();
    onFolderChanged?.();
  }
}
