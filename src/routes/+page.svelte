<script>
  import {
    createAppController,
    WindowControls,
    FullscreenViewer,
    AppModals,
  } from "@modules/shell";
  import { CullWorkspace } from "@modules/culling";
  import { DevelopWorkspace } from "@modules/develop";
  import { ApplePhotosTransfer } from "@modules/apple-photos";

  const app = createAppController();
</script>

<ApplePhotosTransfer
  transfer={app.applePhotos.transfer}
  onCancel={app.cancelApplePhotosTransfer}
  onDismiss={app.dismissApplePhotosTransfer}
/>

<svelte:window onkeydown={app.onKey} onkeyup={app.onKeyUp} onblur={app.onWindowBlur} />

<WindowControls
  currentMode={app.currentMode}
  fullscreen={app.fullscreenState.active}
  isTauri={app.isTauri}
  onClose={app.closeMainWindow}
  onMinimize={app.minimizeMainWindow}
  onZoom={app.zoomMainWindow}
/>

{#if app.currentMode === "cull"}
  <CullWorkspace
    {...app.cullProps}
    bind:currentScrollTop={app.currentScrollTop}
  />
{:else}
  <DevelopWorkspace {...app.developProps} />
{/if}

<a href="/dev-panel" style="display: none;">Prerender Target</a>
<a href="/settings-panel" style="display: none;">Prerender Target</a>

<!-- The one place every long-running operation reports to, regardless of
     which mode (Grid/Develop) is currently showing — an import can finish
     while you're in Develop, and you should still see it. -->
<AppModals />

<FullscreenViewer
  fullscreen={app.fullscreenState.active}
  photo={app.currentPhoto}
  fullscreenUrl={app.fullscreenState.url}
  developPhotoPercent={app.developState.developPhotoPercent}
  sourceOffline={app.sourceOffline}
  onExit={app.exitFullscreen}
/>
