import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow, currentMonitor, availableMonitors } from "@tauri-apps/api/window";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { isTauri } from "@modules/core";
import { placePalette } from "./palettePlacement.js";

/**
 * Palettes configuration specs.
 */
export const PALETTE_SPECS = [
  { label: "develop-panel", url: "/dev-panel", flag: "devPanel", width: 320, height: 850 },
];

/**
 * @param {string} label
 * @param {string | null} [picked]
 */
export function paletteTitle(label, picked = null) {
  if (label === "develop-panel") return picked ? `${picked} — Darkroom` : "Darkroom";
  if (label === "lut-panel") return "LUTs";
  if (label === "preset-panel") return "Presets";
  return "Reveal";
}

/**
 * Beside the main window when there's room (right, then left); a
 * different monitor if neither side fits on the main window's own
 * monitor; the monitor's own right edge as a last resort when there's
 * truly nowhere else. Palettes cascade by index so they don't spawn
 * perfectly stacked.
 *
 * @param {number} index
 * @param {number} panelWidth
 */
export async function computePalettePosition(index, panelWidth) {
  try {
    const main = getCurrentWindow();
    const factor = await main.scaleFactor();
    const outer = await main.outerPosition();
    const size = await main.outerSize();
    const mainMon = await currentMonitor();
    if (!mainMon?.position || !mainMon?.size) return null;

    const onSameMonitor = (/** @type {{ position: { x: number, y: number } }} */ m) =>
      m.position.x === mainMon.position.x && m.position.y === mainMon.position.y;
    const others = (await availableMonitors())
      .filter((m) => !onSameMonitor(m))
      .map((m) => ({ x: m.position.x / factor, y: m.position.y / factor }));

    return placePalette({
      main: { x: outer.x / factor, y: outer.y / factor, width: size.width / factor },
      monitor: { x: mainMon.position.x / factor, width: mainMon.size.width / factor },
      otherMonitors: others,
      index,
      panelWidth,
    });
  } catch (_) {
    return null;
  }
}

/**
 * @param {{ label: string, url: string, flag: string, width: number, height: number }} spec
 * @param {number} index
 * @param {Object} options
 * @param {string} options.currentMode
 * @param {any} options.layouts
 * @param {boolean} [options.spaceLook]
 * @param {string | null} [options.picked]
 * @param {() => void} [options.sendDevStateToPanel]
 * @param {() => void} [options.saveLayouts]
 */
export async function syncPaletteWindow(spec, index, {
  currentMode,
  layouts,
  spaceLook = false,
  picked = null,
  sendDevStateToPanel = () => {},
  saveLayouts = () => {},
}) {
  if (!isTauri) return;
  try {
    let win = await WebviewWindow.getByLabel(spec.label);
    if (currentMode === "dev" && layouts.dev[spec.flag] && layouts.dev.detached && !spaceLook) {
      const pos = await computePalettePosition(index, spec.width);
      if (!win) {
        win = new WebviewWindow(spec.label, {
          url: spec.url,
          title: paletteTitle(spec.label, picked),
          width: spec.width,
          height: spec.height,
          x: pos?.x,
          y: pos?.y,
          resizable: true,
          parent: getCurrentWindow(),
          titleBarStyle: "overlay",
          hiddenTitle: true,
        });
        win.once("tauri://created", () => {
          setTimeout(sendDevStateToPanel, 400);
        });
        win.once("tauri://close-requested", () => {
          layouts.dev[spec.flag] = false;
          saveLayouts();
        });
      } else {
        if (pos) await win.setPosition(new LogicalPosition(pos.x, pos.y));
        await win.show();
        if (index === 0) await win.setFocus();
        sendDevStateToPanel();
      }
    } else {
      if (win) {
        await win.hide();
      }
    }
  } catch (e) {
    console.error("syncPaletteWindow:", spec.label, e);
  }
}

/**
 * Synchronize all configured palette windows.
 * @param {Parameters<typeof syncPaletteWindow>[2]} options
 */
export function syncPalettes(options) {
  PALETTE_SPECS.forEach((spec, i) => syncPaletteWindow(spec, i, options));
}

/**
 * Synchronize the develop panel window specifically.
 * @param {Parameters<typeof syncPaletteWindow>[2]} options
 */
export async function syncDevPanelWindow(options) {
  await syncPaletteWindow(PALETTE_SPECS[0], 0, options);
}
