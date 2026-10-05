/**
 * App Event Handlers Factory.
 *
 * Constructs the unified handlers configuration for Tauri backend events,
 * detached panel IPC, and native menu commands.
 */

import { PREVIEW_PX } from "@modules/develop";

/**
 * Creates the complete handlers bag for `registerAppEventListeners`.
 *
 * @param {{
 *   session: any,
 *   layouts: any,
 *   saveLayouts: () => void,
 *   getCurrentMode: () => "dev" | "cull",
 *   switchMode: (to: "dev" | "cull", opts?: { openDevPanel?: boolean }) => Promise<any> | void,
 *   importState: any,
 *   libraryState: any,
 *   library: any,
 *   cullingState: any,
 *   developState: any,
 *   exportState: any,
 *   saveExportPrefs: () => void,
 *   refreshDirs: (light?: boolean) => Promise<any>,
 *   openDir: (dir: string) => Promise<any> | void,
 *   openPhoto: (path: string) => Promise<any> | void,
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   notify: (msg: string, ms?: number) => void,
 *   setProgress: (progress: any) => void,
 *   updateActivity: (...args: any[]) => void,
 *   activity: any,
 *   getPublishTaskId?: () => string | null,
 *   triggerAiCull: (dir: string, paths: string[]) => Promise<any> | void,
 *   pollCards: () => Promise<any[]>,
 *   importCard: (card: any) => Promise<any> | void,
 *   handleCardMounted: (card: any, opts: any) => void,
 *   handleCardUnmounted: (dcim: string) => void,
 *   handleCullStarted: (payload: any) => void,
 *   handleCullProgress: (payload: any) => void,
 *   handleCullFinished: (payload: any) => void,
 *   handleCullFailed: (payload: any) => void,
 *   refreshLoadedFrames: (rows: any[]) => void,
 *   getSourceOffline: () => boolean,
 *   setSourceOffline: (offline: boolean) => void,
 *   getPhotoPath: () => string | null,
 *   getCurrentFramePath: () => string | undefined,
 *   setGardenAccount: (acc: any) => void,
 *   sendDevStateToPanel: () => void,
 *   sendSettingsToPanel: () => void,
 *   choosePreferenceFolder: (key: string) => Promise<any>,
 *   saveSettingsFromPanel: (prefs: any) => Promise<any>,
 *   openSettings: () => Promise<any>,
 *   edited: (transient?: boolean) => void,
 *   captionEdited: () => void,
 *   tagsEdited: () => void,
 *   applyEngineChange: (engine: string | null) => void,
 *   applyResetRecipe: () => Promise<any>,
 *   toggleCheckLayer: () => void,
 *   clearDevelopment: () => Promise<any>,
 *   scheduleRender: (px: number) => Promise<any> | void,
 *   exportCurrent: (destDir?: string) => Promise<any> | void,
 *   exportSelection: () => Promise<any> | void,
 *   exportToDailyNote: (target?: string) => Promise<any> | void,
 *   chooseExportFolder: (cb: () => void) => Promise<any>,
 *   openInEditor: (path: string) => Promise<any>,
 *   onKey: (event: any) => void,
 *   cycleZoom: (reverse?: boolean) => void,
 *   togglePresetPanel: () => void,
 *   toggleLutPanel: () => void,
 *   toggleDevPanel: () => void,
 *   selectedFrames: (view: any[]) => any[],
 *   getView: () => any[],
 *   getSel: () => number,
 *   applyRecipeToFrames: (recipe: any, targets: any[]) => Promise<any>,
 *   setQueueOpen: () => void,
 *   modalState: any,
 *   preferences: any,
 *   developFromMenu: (path: string, toVault: boolean) => Promise<any>,
 *   toggleAutoImport: () => Promise<any>,
 *   indexRoot: () => Promise<any>,
 * }} deps
 * @returns {import('./appEventListeners.js').AppEventListenerHandlers}
 */
export function createAppEventHandlers(deps) {
  let lastTreeRefresh = 0;
  let lastImportRefresh = 0;
  /** @type {any} */
  let savedRecipeBeforeHover = null;
  /** @type {string | null} */
  let savedEngineBeforeHover = null;
  /** @type {ReturnType<typeof setInterval> | undefined} */
  let sourceWatch;

  return {
    onGardenAccountChanged: (acc) => {
      deps.setGardenAccount(acc);
    },

    onFocusModeChanged: (e) => {
      const mode = deps.getCurrentMode();
      deps.layouts[mode].focus = !!e.enabled;
      deps.saveLayouts();
    },

    onShellPrefsChanged: (e) => {
      deps.importState.autoImport = !!e.auto_import;
      deps.importState.importDir = e.import_dir ?? null;
      const mode = deps.getCurrentMode();
      deps.layouts[mode].focus = !!e.focus_mode;
      deps.saveLayouts();
    },

    onIndexProgress: (e) => {
      if (e.done) {
        deps.libraryState.indexProgress = null;
        deps.refreshDirs();
        return;
      }
      deps.libraryState.indexProgress = e;
      const now = Date.now();
      if (now - lastTreeRefresh > 2500) {
        lastTreeRefresh = now;
        deps.refreshDirs(true);
      }
    },

    onCardsChanged: (cards) => {
      deps.importState.cards = cards;
    },

    onCardMounted: (card) => {
      deps.handleCardMounted(card, {
        onAutoImport: (/** @type {any} */ c) => deps.importCard(c),
      });
    },

    onCardUnmounted: (payload) => {
      deps.handleCardUnmounted(payload?.dcim);
    },

    onImportFirstCardRequested: async () => {
      const available = await deps.pollCards();
      if (available.length) deps.importCard(available[0]);
    },

    onLibrariesChanged: () => deps.refreshDirs(),

    onSourceOffline: () => {
      if (deps.getSourceOffline()) return;
      deps.setSourceOffline(true);
      clearInterval(sourceWatch);
      sourceWatch = setInterval(async () => {
        const probe = deps.getPhotoPath() || deps.getCurrentFramePath();
        if (!probe) return;
        if (await deps.invoke("source_reachable", { path: probe }).catch(() => false)) {
          deps.setSourceOffline(false);
          clearInterval(sourceWatch);
        }
      }, 4000);
    },

    onAppError: (e) => {
      deps.notify(e.message ?? String(e), 5000);
    },

    onImportProgress: async (payload) => {
      deps.setProgress({ verb: "import", ...payload });
      if (payload?.destDir && payload?.dest) {
        const list = deps.importState.importedByFolder.get(payload.destDir) ?? [];
        list.push(payload.dest);
        deps.importState.importedByFolder.set(payload.destDir, list);
      }
      if (payload?.destDir) {
        const now = Date.now();
        if (now - lastImportRefresh > 250) {
          lastImportRefresh = now;
          await deps.refreshDirs(true);
          if (!deps.library.curDir) {
            deps.importState.lastImportedFolder = payload.destDir;
            deps.openDir(payload.destDir);
          } else if (deps.library.curDir === payload.destDir) {
            try {
              const rawRows = await deps.invoke("index_frames", {
                dir: deps.library.curDir,
                minRating: deps.cullingState.minRating,
              });
              const rows = (Array.isArray(rawRows) ? rawRows : []).filter(
                (r) => r.name && !r.name.startsWith(".") && !r.name.startsWith("._")
              );
              if (deps.library.curDir === payload.destDir) deps.refreshLoadedFrames(rows);
            } catch (err) {}
          }
        }
      }
    },

    onImportPreviewReady: (e) => {
      const { dest, version } = e ?? {};
      const frame = deps.library.frames.find((/** @type {any} */ f) => f.path === dest);
      if (frame && version) frame.previewVersion = version;
    },

    onImportStarted: () => {
      deps.setProgress({ verb: "import", done: 0, total: 1, current: "Starting..." });
      deps.importState.importedByFolder = new Map();
    },

    onImportFinished: async (stats) => {
      deps.setProgress(null);
      deps.notify(`Import complete ✓`, 4000);
      if (stats?.folders?.length) {
        const lastFolder = stats.folders[stats.folders.length - 1];
        deps.importState.lastImportedFolder = lastFolder;
        await deps.refreshDirs();
        await deps.openDir(lastFolder);
      }
      if ((deps.cullingState.aiCullMarkStory || deps.cullingState.aiCullExportDesktop) && stats?.folders?.length) {
        for (const imported of stats.folders) {
          const folderPaths = deps.importState.importedByFolder.get(imported);
          if (folderPaths?.length) await deps.triggerAiCull(imported, folderPaths);
        }
      }
    },

    onImportFailed: (e) => {
      deps.importState.lastImportFailureAt = Date.now();
      deps.setProgress(null);
      deps.notify(`Import failed: ${e.message}`, 6000);
    },

    onCullStarted: (payload) => deps.handleCullStarted(payload),
    onCullProgress: (payload) => deps.handleCullProgress(payload),
    onCullFinished: (payload) => deps.handleCullFinished(payload),
    onCullFailed: (payload) => deps.handleCullFailed(payload),

    onExportProgress: (payload) => {
      deps.setProgress({ verb: "export", ...payload });
      if (deps.activity.activeId) {
        deps.updateActivity(deps.activity.activeId, {
          current: payload.current || "Finishing",
          done: payload.done,
          total: payload.total,
          phase: payload.phase || "Developing",
          status: payload.cancelled
            ? "cancelled"
            : payload.done === payload.total
              ? "completed"
              : "running",
        });
      }
      if (payload.done === payload.total || payload.cancelled) {
        setTimeout(() => {
          deps.setProgress(null);
        }, 3000);
      }
    },

    onPublishProgress: (payload) => {
      deps.setProgress({ verb: payload.phase || "publication", ...payload });
      const taskId = deps.getPublishTaskId?.();
      if (taskId) {
        deps.updateActivity(taskId, {
          current: payload.current || payload.phase || "",
          done: payload.done,
          total: payload.total,
        });
      }
    },

    devPanel: {
      onReady: () => deps.sendDevStateToPanel(),
      onRecipeUpdated: (payload) => {
        if (deps.developState.recipe) {
          if (payload?.key) deps.developState.lastEditedKey = payload.key;
          const merged = { ...deps.developState.recipe, ...payload?.recipe };
          deps.developState.recipe = merged;
          if (merged.engine === "rapid" || merged.engine === "spektra") {
            deps.developState.developEngine = merged.engine;
          } else if (merged.engine === "spektrafilm-rs") {
            deps.developState.developEngine = "spektra";
          }
          deps.edited(payload?.transient);
        }
      },
      onEngineUpdated: (engine) => deps.applyEngineChange(engine),
      onCaptionUpdated: (newCaption) => {
        deps.developState.caption = newCaption;
        deps.captionEdited();
      },
      onTagsUpdated: (newTags) => {
        deps.developState.tags = newTags;
        deps.tagsEdited();
      },
      onExport: () => deps.exportCurrent(),
      onExportDesktop: () => deps.exportCurrent(""),
      onExportVault: () => deps.exportToDailyNote(),
      onReset: () => deps.applyResetRecipe(),
      onExportSettingsChanged: (payload) => {
        deps.exportState.edge = payload.exportEdge;
        deps.exportState.border = payload.exportBorder;
        deps.saveExportPrefs();
      },
      onPhotoScaleChanged: (photoScale) => {
        deps.developState.developPhotoPercent = photoScale;
        deps.session.setPhotoSize(deps.developState.developPhotoPercent);
      },
      onChooseExportFolder: () => deps.chooseExportFolder(deps.sendDevStateToPanel),
      onOpenInEditor: (appPath) => deps.openInEditor(appPath),
      onSwitchMode: (mode) => deps.switchMode(mode),
      onForwardKey: (payload) => {
        const p = payload || {};
        deps.onKey({
          key: p.key,
          metaKey: !!p.metaKey,
          ctrlKey: !!p.ctrlKey,
          shiftKey: !!p.shiftKey,
          target: { tagName: "" },
          preventDefault() {},
          stopPropagation() {},
        });
      },
      onZoom: (payload) => deps.cycleZoom(payload?.reverse),
      onClose: () => {
        deps.layouts.dev.devPanel = false;
        deps.saveLayouts();
      },
      onDockRequested: () => {
        deps.layouts.dev.detached = false;
        deps.saveLayouts();
      },
      onSetCheckLayer: (payload) => {
        if (typeof payload?.checkLayer === "string") {
          deps.developState.checkLayer = payload.checkLayer;
          if (deps.developState.checkLayer !== "none") deps.developState.lastActiveCheckLayer = deps.developState.checkLayer;
        }
        deps.sendDevStateToPanel();
      },
      onToggleClipping: (payload) => {
        if (typeof payload?.checkLayer === "string") {
          deps.developState.checkLayer = payload.checkLayer;
          if (deps.developState.checkLayer !== "none") deps.developState.lastActiveCheckLayer = deps.developState.checkLayer;
        } else if (typeof payload?.showClipping === "boolean") {
          deps.developState.checkLayer = payload.showClipping
            ? deps.developState.lastActiveCheckLayer || "clipping"
            : "none";
        } else {
          deps.toggleCheckLayer();
        }
        deps.sendDevStateToPanel();
      },
      onToggleCaption: (payload) => {
        if (typeof payload?.showCaption === "boolean") {
          deps.developState.showCaption = payload.showCaption;
        } else {
          deps.developState.showCaption = !deps.developState.showCaption;
        }
        deps.sendDevStateToPanel();
      },
      onSetZoneMask: (zoneMask) => {
        deps.developState.zoneMaskPreview = zoneMask;
      },
      onPresetPreview: (payload) => {
        const previewRecipe = payload?.recipe;
        if (previewRecipe) {
          if (!savedRecipeBeforeHover && deps.developState.recipe) {
            savedRecipeBeforeHover = { ...deps.developState.recipe };
            savedEngineBeforeHover = deps.developState.developEngine;
          }
          deps.developState.recipe = { ...previewRecipe };
          deps.developState.developEngine = previewRecipe.engine === "rapid" ? "rapid" : "spektra";
          deps.scheduleRender(PREVIEW_PX);
          deps.sendDevStateToPanel();
        } else if (savedRecipeBeforeHover) {
          deps.developState.recipe = { ...savedRecipeBeforeHover };
          deps.developState.developEngine = savedEngineBeforeHover;
          savedRecipeBeforeHover = null;
          savedEngineBeforeHover = null;
          if (deps.developState.developEngine && deps.developState.developEngine !== "none") {
            deps.scheduleRender(PREVIEW_PX);
          } else {
            deps.clearDevelopment();
          }
          deps.sendDevStateToPanel();
        }
      },
      onPresetApply: (payload) => {
        savedRecipeBeforeHover = null;
        savedEngineBeforeHover = null;
        const presetRecipe = payload?.recipe;
        if (!presetRecipe) return;
        const view = deps.getView();
        const sel = deps.getSel();
        const selected = deps.selectedFrames(view);
        const current = view[sel] ? [view[sel]] : selected;
        const targets =
          payload.scope === "selected"
            ? selected.length ? selected : current
            : selected.length > 1 ? selected : current;
        deps.applyRecipeToFrames(presetRecipe, targets);
      },
      onTogglePresetPanel: deps.togglePresetPanel,
      onToggleLutPanel: deps.toggleLutPanel,
      onToggleDevPanel: deps.toggleDevPanel,
    },

    settingsPanel: {
      onReady: () => deps.sendSettingsToPanel(),
      onChooseFolder: (key) => deps.choosePreferenceFolder(key),
      onSave: (prefPayload) => deps.saveSettingsFromPanel(prefPayload),
      onOpenSettings: () => deps.openSettings(),
    },

    menu: {
      onToggleRenderQueue: () => deps.setQueueOpen(),
      onToggleShortcuts: () => deps.modalState.toggleShortcuts(),
      onMenuExport: () => {
        if (deps.getCurrentMode() === "dev" && deps.getPhotoPath() && deps.developState.recipe) {
          deps.exportCurrent();
        } else {
          deps.exportSelection();
        }
      },
      onMenuResetDevelop: async () => {
        if (deps.developState.recipe) {
          deps.developState.recipe = await deps.invoke("default_recipe");
          deps.edited();
        }
      },
      onMenuClearDevelop: () => deps.clearDevelopment(),
      onMenuEnableDevelop: () => {
        if (deps.getCurrentMode() !== "dev") deps.switchMode("dev");
        else deps.edited(false);
      },
      onMenuPublish: () => {
        const view = deps.getView();
        const sel = deps.getSel();
        if (deps.preferences.obsidian_enabled && view[sel]) {
          deps.developFromMenu(view[sel].path, true);
        }
      },
      onDockReopen: () => deps.switchMode("cull"),
      onOpenFile: (filePath) => deps.openPhoto(filePath),
      onToggleAutoImport: () => deps.toggleAutoImport(),
      onAddLibraryFolder: () => deps.indexRoot(),
    },
  };
}
