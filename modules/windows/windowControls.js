import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@modules/core";

export async function closeMainWindow() {
  try {
    await getCurrentWindow().close();
  } catch (error) {
    console.error("closeMainWindow:", error);
  }
}

export async function minimizeMainWindow() {
  try {
    await getCurrentWindow().minimize();
  } catch (error) {
    console.error("minimizeMainWindow:", error);
  }
}

export async function zoomMainWindow() {
  try {
    await getCurrentWindow().toggleMaximize();
  } catch (error) {
    console.error("zoomMainWindow:", error);
  }
}

/**
 * Handle mouse dragging on the main window titlebar/background.
 * @param {MouseEvent} event
 */
export async function startWindowDrag(event) {
  if (!isTauri || event.button !== 0) return;
  const target = event.target;
  if (
    target instanceof Element &&
    target.closest(
      "button, input, select, textarea, a, nav, dialog, [role='dialog'], [role='button'], [role='menu'], [contenteditable='true'], [draggable='true'], .cell, .photo-cell, .roll-cell, .frame, .gap, .composer, .surface, .roll, [role='listitem'], .sidebar-peek, .photo-mat, .curve-editor, svg",
    )
  ) {
    return;
  }
  try {
    await getCurrentWindow().startDragging();
  } catch (error) {
    console.error("startWindowDrag:", error);
  }
}

/**
 * Toggle the OS system appearance (dark/light mode).
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any>, notify?: (msg: string, duration?: number) => void }} deps
 */
export async function toggleAppearance({ invoke, notify = () => {} }) {
  try {
    await invoke("toggle_system_appearance");
  } catch (e) {
    notify(`appearance: ${e}`, 5000);
  }
}

/**
 * Hide the contact sheet window via Tauri IPC.
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} deps
 */
export async function hideWindow({ invoke }) {
  await invoke("hide_contact_sheet").catch(() => {});
}

