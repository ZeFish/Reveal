import { listen as tauriListen } from "@tauri-apps/api/event";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { notify, setProgress, updateActivity, activity } from "@modules/core";
import { initSessionBoot, performAppBoot } from "../shell/appBoot.js";
import { registerAppEventListeners } from "./appEventListeners.js";
import { createAppEventHandlers } from "./appEventHandlers.js";

/**
 * Boots the application session, registers IPC event listeners, and starts card polling.
 *
 * @param {Object} options
 * @param {boolean} options.isTauri
 * @param {typeof tauriListen} [options.listen]
 * @param {typeof tauriInvoke} [options.invoke]
 * @param {any} options.session
 * @param {any} options.layouts
 * @param {() => void} [options.saveLayouts]
 * @param {any} options.modeCtrl
 * @param {any} options.importCtrl
 * @param {any} options.libraryController
 * @param {any} options.navigationCtrl
 * @param {any} options.cullingCtrl
 * @param {any} options.exportCtrl
 * @param {any} options.settingsCtrl
 * @param {any} options.photoMenuCtrl
 * @param {any} options.externalEditorsCtrl
 * @param {any} options.devController
 * @param {any} options.keyboardCtrl
 * @param {any} options.paletteCtrl
 * @param {any} options.modalState
 * @param {any} options.libraryState
 * @param {any} options.library
 * @param {any} options.cullingState
 * @param {any} options.developState
 * @param {any} options.exportState
 * @param {any} options.importState
 * @param {any} options.settingsState
 * @param {any} [options.preferences]
 * @param {(opts: any) => void} options.initCullingState
 * @param {(opts: any) => Promise<any>} options.loadPreferences
 * @param {() => "dev" | "cull"} options.getCurrentMode
 * @param {(mode: "dev" | "cull", opts?: any) => Promise<any> | void} options.switchMode
 * @param {(light?: boolean) => Promise<any>} options.refreshDirs
 * @param {() => void} options.rescan
 * @param {() => string | null} [options.getPublishTaskId]
 * @param {() => boolean} options.getSourceOffline
 * @param {(offline: boolean) => void} options.setSourceOffline
 * @param {() => string | null} options.getPhotoPath
 * @param {() => string | undefined} options.getCurrentFramePath
 * @param {() => any[]} options.getView
 * @param {() => number} options.getSel
 * @param {(view: any[]) => any[]} options.selectedFrames
 * @param {(avail: boolean) => void} options.setGpuAvailable
 * @param {(acc: any) => void} options.setGardenAccount
 * @param {() => void} [options.setQueueOpen]
 * @param {(px: number, live?: boolean) => Promise<any> | void} [options.scheduleRender]
 * @param {(path: string) => Promise<any>} [options.openPhoto]
 * @param {(cb?: any) => Promise<any>} [options.chooseExportFolder]
 * @param {() => void} [options.togglePresetPanel]
 * @param {() => void} [options.toggleLutPanel]
 * @param {(msg: string) => void} [options.log]
 * @returns {() => void} Cleanup function to stop listeners and timers.
 */
export function startAppLifecycle(options) {
  const {
    isTauri,
    listen = tauriListen,
    invoke = tauriInvoke,
    session,
    layouts,
    saveLayouts = () => {},
    modeCtrl,
    importCtrl,
    libraryController,
    navigationCtrl,
    cullingCtrl,
    exportCtrl,
    settingsCtrl,
    photoMenuCtrl,
    externalEditorsCtrl,
    devController,
    keyboardCtrl,
    paletteCtrl,
    modalState,
    libraryState,
    library,
    cullingState,
    developState,
    exportState,
    importState,
    settingsState,
    preferences = settingsCtrl?.preferences,
    initCullingState,
    loadPreferences,
    getCurrentMode,
    switchMode,
    refreshDirs,
    rescan,
    getPublishTaskId,
    getSourceOffline,
    setSourceOffline,
    getPhotoPath,
    getCurrentFramePath,
    getView,
    getSel,
    selectedFrames,
    setGpuAvailable,
    setGardenAccount,
    setQueueOpen = () => {},
    scheduleRender = (px) => devController?.scheduleRender?.(px),
    openPhoto = (p) => navigationCtrl?.openPhoto?.(p),
    chooseExportFolder = (cb) => exportCtrl?.chooseExportFolder?.(cb),
    togglePresetPanel = () => {},
    toggleLutPanel = () => {},
    log,
  } = options;

  initSessionBoot({
    session,
    layouts,
    initCullingState,
    exportState,
  });
  if (modeCtrl) modeCtrl.currentMode = "cull";

  if (!isTauri) return () => {};

  /** @type {Promise<() => void>[]} */
  const wired = [];
  /** @param {string} event @param {(payload: any) => void} handler */
  const on = (event, handler) => void wired.push(listen(event, handler));

  /** @type {ReturnType<typeof setInterval> | undefined} */
  let cardInterval;

  (async () => {
    await performAppBoot({
      invoke,
      developState,
      loadExternalEditors: () => externalEditorsCtrl.loadExternalEditors(),
      refreshDirs,
      session,
      library,
      openDir: (d, rm, rs) => navigationCtrl.openDir(d, rm, rs),
      rescan,
      layouts,
      currentMode: getCurrentMode(),
      importState,
      loadPreferences,
      settingsState,
      exportState,
      initCullingState,
      setGpuAvailable,
      setGardenAccount,
      openPhoto: (p) => openPhoto(p),
      openFolder: (f) => navigationCtrl.openFolder(f),
      log,
    });

    registerAppEventListeners(
      on,
      createAppEventHandlers({
        session,
        layouts,
        saveLayouts,
        getCurrentMode,
        switchMode,
        importState,
        libraryState,
        library,
        cullingState,
        developState,
        exportState,
        saveExportPrefs: () => exportCtrl?.saveExportPrefs?.(),
        refreshDirs,
        openDir: (d) => navigationCtrl.openDir(d),
        openPhoto: (p) => openPhoto(p),
        invoke,
        notify,
        setProgress,
        updateActivity,
        activity,
        getPublishTaskId,
        triggerAiCull: (d, p) => cullingCtrl.triggerAiCull(d, p),
        pollCards: () => importCtrl.pollCards(),
        importCard: (c) => importCtrl.importCard(c),
        handleCardMounted: (c, opts) => importCtrl.handleCardMounted(c, opts),
        handleCardUnmounted: (dcim) => importCtrl.handleCardUnmounted(dcim),
        handleCullStarted: (payload) => cullingCtrl.handleCullStarted?.(payload),
        handleCullProgress: (payload) => cullingCtrl.handleCullProgress?.(payload),
        handleCullFinished: (payload) => cullingCtrl.handleCullFinished?.(payload),
        handleCullFailed: (payload) => cullingCtrl.handleCullFailed?.(payload),
        refreshLoadedFrames: (rows) => libraryController.refreshLoadedFrames?.(rows),
        getSourceOffline,
        setSourceOffline,
        getPhotoPath,
        getCurrentFramePath,
        setGardenAccount,
        sendDevStateToPanel: () => paletteCtrl.sendDevStateToPanel(),
        sendSettingsToPanel: () => settingsCtrl.sendSettingsToPanel(),
        choosePreferenceFolder: (key) => settingsCtrl.choosePreferenceFolder(key),
        saveSettingsFromPanel: (prefs) => settingsCtrl.saveSettingsFromPanel(prefs),
        openSettings: () => settingsCtrl.openSettings(),
        edited: (transient) => devController.edited(transient),
        captionEdited: () => devController.captionEdited(),
        tagsEdited: () => devController.tagsEdited(),
        applyEngineChange: (engine) => devController.applyEngineChange(engine),
        applyResetRecipe: () => devController.applyResetRecipe(),
        toggleCheckLayer: () => devController.toggleCheckLayer(),
        clearDevelopment: () => devController.clearDevelopment(),
        scheduleRender,
        exportCurrent: () => exportCtrl.exportCurrent(),
        exportSelection: () => exportCtrl.exportSelection(),
        exportToDailyNote: () => exportCtrl.exportToDailyNote(),
        chooseExportFolder,
        openInEditor: (/** @type {string} */ path) => externalEditorsCtrl.openInEditor(path),
        onKey: (/** @type {any} */ e) => keyboardCtrl.onKey(e),
        cycleZoom: () => modeCtrl.cycleZoom(),
        togglePresetPanel,
        toggleLutPanel,
        toggleDevPanel: () => paletteCtrl.toggleDevPanel(),
        selectedFrames,
        getView,
        getSel,
        applyRecipeToFrames: (/** @type {any} */ r, /** @type {any[]} */ f) => devController.applyRecipeToFrames(r, f),
        setQueueOpen,
        modalState,
        preferences,
        developFromMenu: (/** @type {string} */ p, /** @type {boolean} */ toVault) => photoMenuCtrl.developFromMenu(p, toVault),
        toggleAutoImport: () => importCtrl.toggleAutoImport(),
        indexRoot: () => libraryController.indexRoot(),
      }),
    );

    importCtrl.pollCards();
    cardInterval = setInterval(() => importCtrl.pollCards(), 5000);
  })();

  return () => {
    if (cardInterval) clearInterval(cardInterval);
    for (const pending of wired) {
      pending.then((stop) => stop()).catch(() => {});
    }
  };
}
