<script>
  import { untrack } from "svelte";
  import DevelopView from "./DevelopView.svelte";
  import DevelopPanel from "./DevelopPanel.svelte";
  import PhotoMarks from "./PhotoMarks.svelte";
  import { showsDockedPanel } from "./dockedPanel.js";
  // Module singletons, imported rather than passed down: `bind:` into a prop the
  // parent did not declare bindable trips Svelte's ownership checks.
  import { developState } from "./developState.svelte.js";
  import { exportState } from "../export/exportState.svelte.js";

  /**
   * @typedef {Object} Props
   * @property {any} layouts
   * @property {() => void} saveLayouts
   * @property {any} session
   * @property {boolean} isTauri
   * @property {string | null} photoPath
   * @property {string | null} picked
   * @property {string | null} imgUrl
   * @property {string} status
   * @property {number} currentRating
   * @property {boolean} [spaceLook] Space quick look: the photo developed, no panel around it
   * @property {boolean} [inStory]
   * @property {(n: number) => void} [onRate]
   * @property {() => void} [onToggleStory]
   * @property {string | null} [beforeUrl] the photo as the camera shot it
   * @property {() => void} [onCompareDown] before / after: the button goes down
   * @property {() => void} [onCompareUp] before / after: the button comes up
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
    layouts,
    saveLayouts,
    session,
    isTauri,
    photoPath,
    picked,
    imgUrl,
    status,
    currentRating,
    spaceLook = false,
    inStory = false,
    onRate = () => {},
    onToggleStory = () => {},
    beforeUrl = null,
    onCompareDown = () => {},
    onCompareUp = () => {},
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

  // The Crop tab renders the WHOLE frame (the overlay is drawn on it); every other tab
  // renders the cropped one. Nothing re-rendered when the tab changed, so leaving the
  // Crop tab kept showing the uncropped picture: the crop looked as if it had not
  // stuck. Draw again whenever the tab crosses that line.
  /** @type {boolean | null} */
  let wasInCropTab = null;
  $effect(() => {
    const inCrop = developState.dockedActiveTab === "crop";
    untrack(() => {
      if (wasInCropTab !== null && wasInCropTab !== inCrop) devController.rerender?.();
      wasInCropTab = inCrop;
    });
  });
  // Leaving Develop for the grid ends the crop tool. Coming back with the Crop tab still
  // active showed the uncropped frame with the overlay on it instead of the result.
  $effect(() => () => {
    if (developState.dockedActiveTab === "crop") developState.dockedActiveTab = "dev";
  });

  const showDockedPanel = $derived(
    showsDockedPanel({
      hasRecipe: Boolean(developState.recipe),
      devPanel: Boolean(layouts.dev.devPanel),
      detached: Boolean(layouts.dev.detached),
      isTauri,
      spaceLook,
    }),
  );
</script>

<div
  class="app"
  role="presentation"
  style="grid-template-columns: {showDockedPanel ? '1fr 22rem' : '1fr'};"
  onmousedown={onStartWindowDrag}
>
  <div class="stage">
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
      showBefore={developState.showBefore}
      {beforeUrl}
      {onPhotoPointerDown}
      {onPhotoPointerMove}
      {onPhotoPointerUp}
      showCropOverlay={showDockedPanel && developState.dockedActiveTab === "crop"}
      {onCropChange}
    />
    {#if photoPath && !developState.dockedActiveTab?.startsWith("crop")}
      <div class="marks-slot">
        <PhotoMarks
          rating={currentRating}
          {inStory}
          readOnly={photoPath.startsWith("apple-photos://")}
          {onRate}
          {onToggleStory}
          comparing={developState.showBefore}
          {onCompareDown}
          {onCompareUp}
          checkLayer={developState.checkLayer !== "none" ? developState.checkLayer : developState.showClipping ? "clipping" : "none"}
          onToggleCheckLayer={() => devController.toggleCheckLayer()}
          showCaption={developState.showCaption}
          onToggleCaption={devController.dockedToggleCaptionOverlay}
        />
      </div>
    {/if}
  </div>
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
    border-radius: var(--window-radius);
  }

  .stage {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    height: 100%;
    overflow: hidden;
  }

  .marks-slot {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 12px;
    z-index: 8;
    display: flex;
    justify-content: center;
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--transition-fast);
  }

  .stage:hover .marks-slot,
  .stage:focus-within .marks-slot {
    opacity: 1;
  }

  .docked-panel-frame {
    min-height: 0;
  }
</style>
