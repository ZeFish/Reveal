/**
 * Restores initial user session layout and export preferences synchronously before paint.
 *
 * @param {Object} options
 * @param {any} options.session
 * @param {Record<string, any>} options.layouts
 * @param {(opts: any) => void} options.initCullingState
 * @param {any} options.exportState
 */
export function initSessionBoot({ session, layouts, initCullingState, exportState }) {
  const parsed = session.modeLayouts?.();
  if (parsed) {
    Object.assign(layouts, parsed);
  }

  initCullingState({
    gridPrefs: session.gridPrefs?.(),
    gridLayout: session.gridLayout?.(),
  });

  const x = session.exportPrefs?.();
  if (x) {
    if ([0, 4096, 2048, 1600, 1024].includes(x.edge)) exportState.edge = x.edge;
    if (typeof x.border === "boolean") exportState.border = x.border;
    if (typeof x.folder === "string") exportState.folder = x.folder;
  }
}

/**
 * Executes the asynchronous startup sequence when running inside Tauri.
 *
 * @param {Object} options
 * @param {(cmd: string, args?: any) => Promise<any>} options.invoke
 * @param {any} options.developState
 * @param {() => Promise<any>} options.loadExternalEditors
 * @param {() => Promise<any>} options.refreshDirs
 * @param {any} options.session
 * @param {any} options.library
 * @param {(dir: string, restoreMode?: boolean, restoreSession?: boolean) => Promise<any>} options.openDir
 * @param {() => void} options.rescan
 * @param {Record<string, any>} options.layouts
 * @param {string} options.currentMode
 * @param {any} options.importState
 * @param {(opts: { invoke: any }) => Promise<any>} options.loadPreferences
 * @param {any} options.settingsState
 * @param {any} options.exportState
 * @param {(opts: any) => void} options.initCullingState
 * @param {(available: boolean) => void} options.setGpuAvailable
 * @param {(account: any) => void} options.setGardenAccount
 * @param {(path: string) => Promise<any>} options.openPhoto
 * @param {(path: string) => Promise<any>} options.openFolder
 * @param {(msg: string) => void} [options.log]
 */
export async function performAppBoot({
  invoke,
  developState,
  loadExternalEditors,
  refreshDirs,
  session,
  library,
  openDir,
  rescan,
  layouts,
  currentMode,
  importState,
  loadPreferences,
  settingsState,
  exportState,
  initCullingState,
  setGpuAvailable,
  setGardenAccount,
  openPhoto,
  openFolder,
  log,
}) {
  if (typeof window !== "undefined") {
    log?.(`viewport ${window.innerWidth}x${window.innerHeight} dpr=${window.devicePixelRatio}`);
  }

  developState.recipe = await invoke("default_recipe");
  const profiles = await invoke("list_profiles").catch(() => []);
  await loadExternalEditors();

  const profileList = Array.isArray(profiles) ? profiles : [];
  developState.films = profileList.filter((/** @type {any} */ p) => p.stage === "filming");
  developState.papers = profileList.filter((/** @type {any} */ p) => p.stage === "printing");
  const luts = await invoke("list_luts").catch(() => []);
  developState.luts = Array.isArray(luts) ? luts : [];
  developState.engines = await invoke("list_engines").catch(() => []);

  const gpuAvailable = await invoke("gpu_available").catch(() => false);
  setGpuAvailable(gpuAvailable);

  developState.developDefaults = await invoke("default_recipe").catch(() => null);
  await refreshDirs();

  const lastDir = session.lastDirectory?.() || library.dirs?.[library.dirs.length - 1]?.dir;
  if (lastDir) {
    await openDir(lastDir, false, true);
  }

  if (library.root && !library.dirs?.length) {
    rescan();
  }

  const shellPrefs = (await invoke("load_shell_prefs").catch(() => ({}))) || {};
  if (layouts[currentMode]) {
    layouts[currentMode].focus = !!shellPrefs.focus_mode;
  }
  importState.autoImport = !!shellPrefs.auto_import;

  const savedPreferences = await loadPreferences({ invoke });
  settingsState.set(savedPreferences);
  exportState.folder = savedPreferences.export_folder ?? "";
  initCullingState({ preferences: savedPreferences });

  if (shellPrefs.garden_username) {
    setGardenAccount({
      signed_in: true,
      username: shellPrefs.garden_username,
      tier: shellPrefs.garden_tier,
      notes_count: 0,
      total_views: 0,
    });
  }

  invoke("garden_refresh")
    .then((info) => setGardenAccount(info))
    .catch(() => {});

  const auto = (await invoke("autoload_path").catch(() => null)) || (await invoke("take_open_file").catch(() => null));
  if (auto) {
    if (auto.endsWith("/") || !auto.includes(".")) {
      await openFolder(auto);
    } else {
      await openPhoto(auto);
    }
  }
}
