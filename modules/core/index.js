/**
 * Core foundation, stores, and utilities for Reveal.
 *
 * Provides shared state singletons (selection, activity, session),
 * the Photo domain model, Tauri IPC wrappers, manual docs links,
 * app theming, and shared UI primitives (Icon, ManualLink).
 */

// --- Stores ---
export {
  selection,
  positionIn,
  anchorIn,
  focusAt,
  selectOnly,
  selectGridItem,
  selectRange,
  toggle,
  selectAll,
  setAnchor,
  clearSelection,
  setSelection,
  selectedFrames,
} from "./stores/selection.svelte.js";

export {
  activity,
  notify,
  hold,
  dismiss,
  startActivity,
  updateActivity,
  setActive,
  releaseActive,
  setQueueOpen,
  setProgress,
  patchProgress,
  advanceProgress,
} from "./stores/activity.svelte.js";

export {
  session,
  openFolderSession,
  DEFAULT_PHOTO_SIZE,
  PHOTO_SIZE_MIN,
  PHOTO_SIZE_MAX,
} from "./stores/session.js";

export { virtualCollections, SMART_COLLECTIONS } from "./stores/virtualCollections.svelte.js";

// --- Domain Models ---
import { Photo } from "./models/Photo.js";
import { Source } from "./models/Source.js";
import { Collection } from "./models/Collection.js";
import { Destination } from "./models/Destination.js";
export { Photo, Source, Collection, Destination };
export const thumbUrl = Photo.thumb;

// --- Tauri IPC Wrappers ---
export { isTauri, ping } from "./tauri.js";

// --- Manual Docs Links ---
export {
  MANUAL_URL,
  manualUrl,
  openManual,
  TAB_PAGES,
} from "./manual.js";

// --- App Theming & Fonts ---
export {
  THEME_IDS,
  DEFAULT_THEME,
  GARDEN_THEMES,
  loadFontPackages,
  applyTheme,
  applyFolderTheme,
  DEFAULT_TEXT_SIZE,
  TEXT_SIZES,
  applyTextSize,
} from "./theme.js";

// --- UI Primitives ---
export { default as Icon } from "./components/Icon.svelte";
export { default as ManualLink } from "./components/ManualLink.svelte";
export { icons } from "./icons.js";
