/**
 * Core type definitions for Reveal.
 */

/**
 * A catalogue frame row, as returned by `index_frames` / `list_dir`.
 * @typedef {Object} Frame
 * @property {string} path
 * @property {string} [name]
 * @property {number} [rating]
 * @property {number} [width]
 * @property {number} [height]
 * @property {string | number} [capture_at]
 * @property {number} [aperture]
 * @property {string} [shutter]
 * @property {number} [iso]
 * @property {number} [focal_mm]
 * @property {string} [captured_at]
 * @property {string} [make]
 * @property {string} [model]
 */

/**
 * A directory row from `index_dirs`.
 * @typedef {Object} Dir
 * @property {string} dir
 */

/**
 * A removable card, as returned by `find_cards`.
 * @typedef {Object} Card
 * @property {string} [volume]
 * @property {string} name
 * @property {string} dcim
 * @property {number} raw_count
 */

/**
 * A develop recipe — the engine_settings blob persisted to sidecars.
 * @typedef {Record<string, any>} Recipe
 */

/**
 * Long-running operation progress (import/export/publish/move).
 * @typedef {Object} Progress
 * @property {string} verb
 * @property {number} done
 * @property {number} total
 * @property {string} [current]
 * @property {string} [path]
 */

/**
 * The photo grid context-menu position/target.
 * @typedef {Object} PhotoMenu
 * @property {Frame} frame
 * @property {number} x
 * @property {number} y
 */

/**
 * A film/paper profile from `list_profiles`.
 * @typedef {Object} Profile
 * @property {string} stage
 * @property {string} name
 * @property {string} label
 */

/**
 * An engine registry entry from `list_engines`.
 * @typedef {Object} EngineInfo
 * @property {string} id
 * @property {string} label
 * @property {any} control_groups
 */

/**
 * One entry in the activity queue.
 * @typedef {Object} Activity
 * @property {string} id
 * @property {string} kind e.g. "import" | "export" | "cull" | "publish" | "move" | "develop"
 * @property {string} label
 * @property {string} current
 * @property {number} done
 * @property {number} total
 * @property {string} phase
 * @property {string} status "running" | "completed" | "failed" | "cancelled"
 * @property {string} timestamp
 */

/**
 * Sidebar Garden account state.
 * @typedef {Object} GardenAccount
 * @property {boolean} signed_in
 * @property {string} [username]
 * @property {string} [tier]
 * @property {number} notes_count
 * @property {number} total_views
 */

/**
 * A developed RGBA preview buffer returned by `develop_preview_rgba`.
 * @typedef {Object} RgbaPreview
 * @property {number} width
 * @property {number} height
 * @property {number} renderMs
 * @property {Uint8ClampedArray<ArrayBuffer>} rgba
 */

/**
 * Extended Window interface for Tauri dev/logging.
 * @typedef {Window & typeof globalThis & {
 *   __log?: (...args: any[]) => void;
 *   currentMonitor?: () => Promise<any> | any;
 * }} RevealWindow
 */

export {};
