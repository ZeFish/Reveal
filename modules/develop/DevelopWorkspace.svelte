<script>
  import DevelopView from "./DevelopView.svelte";
  import DevelopPanel from "./DevelopPanel.svelte";

  /**
   * @typedef {Object} Props
   * @property {any} developState
   * @property {any} exportState
   * @property {any} layouts
   * @property {() => void} saveLayouts
   * @property {any} session
   * @property {boolean} isTauri
   * @property {string | null} photoPath
   * @property {string | null} picked
   * @property {string | null} imgUrl
   * @property {string} status
   * @property {number} currentRating
   * @property {boolean} sourceOffline
   * @property {any[]} installedEditors
   * @property {string} zoomMode
   * @property {boolean} panning
   * @property {(e: any) => void} onPhotoPointerDown
   * @property {(e: any) => void} onPhotoPointerMove
   * @property {(e: any) => void} onPhotoPointerUp
   * @property {(live: boolean) => void} onCropChange
   * @property {(e: MouseEvent) => void} onStartWindowDrag
   * @property {any} devController
   * @property {any} exportCtrl
   * @property {any} paletteCtrl
   * @property {(path: string) => Promise<any>} openInEditor
   */

  /** @type {Props} */
  let {
    developState,
    exportState,
    layouts,
    saveLayouts,
    session,
    isTauri,
    photoPath,
    picked,
    imgUrl,
    status,
    currentRating,
    sourceOffline,
    installedEditors,
    zoomMode,
    panning,
    onPhotoPointerDown,
    onPhotoPointerMove,
    onPhotoPointerUp,
    onCropChange,
    onStartWindowDrag,
    devController,
    exportCtrl,
    paletteCtrl,
    openInEditor,
  } = $props();

  const showDockedPanel = $derived(
    Boolean(developState.recipe) &&
    Boolean(layouts.dev.devPanel) &&
    (!isTauri || !layouts.dev.detached)
  );
</script>

<div
  class="app"
  role="presentation"
  style="grid-template-columns: {showDockedPanel ? '1fr 22rem' : '1fr'};"
  onmousedown={onStartWindowDrag}
>
  <DevelopView
    {picked}
    imgUrl={imgUrl ?? undefined}
    useCanvas={developState.useCanvas}
    canvasVersion={developState.canvasVersion}
    showClipping={developState.showClipping}
    checkLayer={developState.checkLayer}
    bind:zoneMask={developState.zoneMaskPreview}
    onSelectCheckLayer={devController.toggleCheckLayer}
    caption={developState.caption}
    showCaption={developState.showCaption}
    recipe={developState.recipe}
    renderAspect={developState.renderAspect}
    bind:canvasEl={developState.canvasEl}
    bind:imgFailed={developState.imgFailed}
    bind:histogram={developState.histogram}
    bind:scopes={developState.scopes}
    {status}
    inflight={developState.inflight}
    pendingPx={developState.pendingPx}
    {zoomMode}
    {panning}
    developPhotoPercent={developState.developPhotoPercent}
    {sourceOffline}
    {onPhotoPointerDown}
    {onPhotoPointerMove}
    {onPhotoPointerUp}
    showCropOverlay={showDockedPanel && developState.dockedActiveTab === "crop"}
    {onCropChange}
  />
  {#if showDockedPanel}
    <div class="docked-panel-frame pane">
      <DevelopPanel
        {photoPath}
        {picked}
        bind:recipe={developState.recipe}
        bind:activeTab={developState.dockedActiveTab}
        bind:activeZone={developState.dockedActiveZone}
        onSetZoneMask={(/** @type {any} */ mask) => { developState.zoneMaskPreview = mask; }}
        developEngine={developState.developEngine}
        renderMs={developState.renderMs}
        {status}
        {installedEditors}
        bind:exportEdge={exportState.edge}
        bind:exportBorder={exportState.border}
        exportFolder={exportState.folder}
        films={developState.films}
        papers={developState.papers}
        luts={developState.luts}
        engines={developState.engines}
        bind:caption={developState.caption}
        bind:tags={developState.tags}
        rating={currentRating}
        bind:publishing={developState.devPublishing}
        bind:publishStatus={developState.devPublishStatus}
        histogram={developState.histogram}
        scopes={developState.scopes}
        bind:photoScale={developState.developPhotoPercent}
        onPhotoScaleChanged={(/** @type {number} */ percent) => {
          developState.developPhotoPercent = percent;
          session.setPhotoSize(percent);
        }}
        showClipping={developState.showClipping}
        checkLayer={developState.checkLayer}
        onSelectCheckLayer={devController.toggleCheckLayer}
        toggleClipping={devController.dockedToggleClipping}
        showCaption={developState.showCaption}
        toggleCaptionOverlay={devController.dockedToggleCaptionOverlay}
        edited={devController.dockedEdited}
        resetOne={devController.resetOne}
        addLutLayer={devController.addLutLayer}
        removeLutLayer={devController.removeLutLayer}
        updateLutOpacity={devController.updateLutOpacity}
        setLutFile={devController.setLutFile}
        engineChanged={devController.dockedEngineChanged}
        resetRecipe={devController.applyResetRecipe}
        hidePanel={devController.dockedHidePanel}
        onCaptionEdited={devController.captionEdited}
        onTagsEdited={devController.tagsEdited}
        onExportSettingsChanged={devController.dockedExportSettingsChanged}
        onExport={exportCtrl.exportCurrent}
        onExportDaily={exportCtrl.exportToDailyNote}
        onChooseExportFolder={() => exportCtrl.chooseExportFolder(paletteCtrl.sendDevStateToPanel)}
        onOpenInEditor={openInEditor}
        detached={false}
        onToggleDetached={() => {
          layouts.dev.detached = true;
          saveLayouts();
        }}
      />
    </div>
  {/if}
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 1fr 22rem;
    height: 100vh;
    position: relative;
    z-index: 1;
    background: var(--canvas);
  }

  .docked-panel-frame {
    min-height: 0;
  }
</style>
