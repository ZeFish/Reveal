/**
 * Sidebar Controller.
 *
 * Coordinates sidebar visibility, slide-out peek hovering, and layout toggling.
 */

/**
 * Creates a reactive sidebar controller managing peek preview and visibility.
 *
 * @param {{
 *   getLayouts: () => any,
 *   getCurrentMode: () => "cull" | "dev",
 *   getRoot: () => string | null,
 *   isApplePhotosSupported: () => boolean,
 *   saveLayouts?: () => void,
 * }} deps
 */
export function createSidebarController(deps) {
  const {
    getLayouts,
    getCurrentMode,
    getRoot,
    isApplePhotosSupported,
    saveLayouts = () => {},
  } = deps;

  let peek = $state(false);
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let peekTimer = undefined;

  function openPeek() {
    const layouts = getLayouts();
    const mode = getCurrentMode();
    const root = getRoot();
    const photosSupported = isApplePhotosSupported();
    if ((!root && !photosSupported) || layouts[mode]?.sidebar) return;
    clearTimeout(peekTimer);
    peek = true;
  }

  function schedulePeekClose() {
    clearTimeout(peekTimer);
    peekTimer = setTimeout(() => {
      peek = false;
    }, 380);
  }

  function closePeek() {
    clearTimeout(peekTimer);
    peek = false;
  }

  function toggleSidebar() {
    closePeek();
    const layouts = getLayouts();
    const mode = getCurrentMode();
    if (layouts[mode]) {
      layouts[mode].sidebar = !layouts[mode].sidebar;
      saveLayouts();
    }
  }

  return {
    get peek() {
      return peek;
    },
    get isVisible() {
      const layouts = getLayouts();
      const mode = getCurrentMode();
      const root = getRoot();
      const photosSupported = isApplePhotosSupported();
      return (Boolean(root) || photosSupported) && Boolean(layouts[mode]?.sidebar);
    },
    openPeek,
    schedulePeekClose,
    closePeek,
    toggleSidebar,
  };
}
