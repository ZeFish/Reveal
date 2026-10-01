<script>
  // The develop panel's INTERFACE — a 1:1 port of the Swift `DevPanel.swift` +
  // `Controls.swift` control language: DIN section titles with chevrons
  // (collapsible, remembered), SliderField rows (88px DIN label, thin ink
  // track, round thumb, mono value), StyledMenu pills, StyledToggle capsule,
  // and the I/D/E shape: INFO / DEVELOP / EXPORT.
  //
  // Deliberately unaware of HOW it's hosted: docked inline (bound straight to
  // the main window's own reactive state — mutating `recipe` here just works,
  // no extra step) or detached into its own OS window (the caller mirrors
  // this component's bindable state locally and relays changes over Tauri
  // events — see routes/dev-panel/+page.svelte). Every prop below that ends
  // in an emit today (edited, resetOne, engineChanged, the LUT layer
  // functions, resetRecipe, toggleClipping, hidePanel) is intentionally
  // owned by the caller, not this component, because what happens after the
  // mutation is exactly the thing that differs between the two hosts.
  import { invoke } from "@tauri-apps/api/core";
  import { DEFAULT_PHOTO_SIZE } from "$lib/session.js";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";
  import InfoBlock from "./tabs/InfoBlock.svelte";
  import { openManual, TAB_PAGES } from "$lib/manual.js";
  import DevTab from "./tabs/DevTab.svelte";
  import CropTab from "./tabs/CropTab.svelte";
  import PresetTab from "./tabs/PresetTab.svelte";
  import ExportTab from "./tabs/ExportTab.svelte";

  /** @typedef {{ name: string, label: string }} FilmOrPaper */
  /** @typedef {{ id: string, label: string, control_groups?: any[] }} EngineInfo */
  /** @typedef {{ aperture?: number, shutter?: string, iso?: number, focal_mm?: number, captured_at?: string, make?: string, model?: string, width?: number, height?: number }} ExifInfo */
  /** @typedef {Record<string, any>} Recipe */

  let {
    photoPath = null,
    picked = null,
    recipe = $bindable(null),
    developEngine = null,
    renderMs = null,
    status = "",
    /** @type {[string, string][]} */
    installedEditors = [],
    exportEdge = $bindable(2048),
    exportBorder = $bindable(false),
    exportFolder = "",
    /** @type {FilmOrPaper[]} */
    films = [],
    /** @type {FilmOrPaper[]} */
    papers = [],
    luts = [],
    /** @type {EngineInfo[]} */
    engines = [],
    caption = $bindable(""),
    /** @type {string[]} */
    tags = $bindable([]),
    rating = 0,
    publishing = $bindable(false),
    publishStatus = $bindable(""),
    showClipping = false,
    toggleClipping = () => {},
    showCaption = false,
    toggleCaptionOverlay = () => {},
    /** @param {boolean} [transient] @param {string} [key] */
    edited = () => {},
    /** @param {string} key @param {number} [index] */
    resetOne = () => {},
    /** @param {string} stage */
    addLutLayer = () => {},
    /** @param {string} stage @param {number} index */
    removeLutLayer = () => {},
    /** @param {string} stage @param {number} index @param {number | string} value */
    updateLutOpacity = () => {},
    /** @param {string} stage @param {number} index @param {string} name */
    setLutFile = () => {},
    /** @param {string} value */
    engineChanged = () => {},
    resetRecipe = () => {},
    hidePanel = () => {},
    onCaptionEdited = () => {},
    onTagsEdited = () => {},
    /** @param {{exportEdge: number, exportBorder: boolean}} settings */
    onExportSettingsChanged = () => {},
    onExport = () => {},
    onExportDaily = () => {},
    onChooseExportFolder = () => {},
    /** @param {string} appPath */
    onOpenInEditor = () => {},
    // Whether THIS host is the detached OS window (true) or docked inline
    // (false) — purely cosmetic here (which icon/tooltip to show); the two
    // hosts wire onToggleDetached to opposite ends of the same flag.
    detached = false,
    onToggleDetached = () => {},
    // Bindable so the docked host can know when Crop is open (to show the
    // crop grid/rotation preview on the photo itself, in DevelopView) —
    // this panel's own tabs otherwise never leave this component.
    activeTab = $bindable("dev"),
    // One-way down from wherever the photo canvas actually lives (docked:
    // DevelopView in the same window; detached: relayed over main-dev-state
    // — see routes/dev-panel/+page.svelte) straight into DevTab's histogram.
    histogram = null,
    scopes = null,
    // The Photo Size slider's own value — bindable for instant local echo
    // (mirrors exportEdge/exportBorder's pattern), onPhotoScaleChanged is
    // what actually makes it stick: a no-op when docked (this panel shares
    // memory with the window that owns the photo), a relay emit when
    // detached (see routes/dev-panel/+page.svelte).
    photoScale = $bindable(DEFAULT_PHOTO_SIZE),
    onPhotoScaleChanged = () => {},
  } = $props();

  // developEngine holds the Rust engine id ("spektra" | "rapid" | null).
  const activeEngine = $derived(engines.find((e) => e.id === developEngine));

  // EXIF arrives per photo — cheap metadata-only read, no pixel decode. Pure
  // local lookup, identical in every host, so it lives here rather than
  // being fetched (and passed down) twice.
  /** @type {ExifInfo | null} */
  let exif = $state(null);
  $effect(() => {
    const p = photoPath;
    if (!isTauri) return;
    exif = null;
    if (p) {
      invoke("frame_info", { path: p })
        .then((i) => {
          if (p === photoPath) exif = i;
        })
        .catch(() => {});
    }
  });

  // "ƒ5.6  1/60  ISO 1000  23mm" — the Swift `exposure.spec` line.
  const exifLine = $derived.by(() => {
    if (!exif) return "";
    const parts = [];
    if (exif.aperture) parts.push(`ƒ${Number(exif.aperture.toFixed(1))}`);
    if (exif.shutter) parts.push(exif.shutter);
    if (exif.iso) parts.push(`ISO ${exif.iso}`);
    if (exif.focal_mm) parts.push(`${Math.round(exif.focal_mm)}mm`);
    return parts.join("   ");
  });
</script>

<div class="panel">
  <!-- STICKY TOP ZONE -->
  <div class="sticky-top">
    <header data-tauri-drag-region>
      <span class="din title">{picked ?? "—"}</span>
      <button
        class="header-util-btn ghost"
        onclick={() => onToggleDetached()}
        title={detached ? "Re-dock the panel" : "Detach into its own window"}
      >
        <Icon name={detached ? "arrows-in-simple" : "arrow-square-out"} size="12px" />
      </button>
      <button
        class="header-util-btn ghost"
        aria-pressed={showClipping}
        onclick={() => toggleClipping()}
        title="Clipping warning (highlights & shadows)"
      >
        <Icon name="circle-half" size="12px" />
        {#if showClipping}
          <span class="clip-indicator"></span>
        {/if}
      </button>
      <button
        class="header-util-btn ghost"
        aria-pressed={showCaption}
        onclick={() => toggleCaptionOverlay()}
        title="Show caption at bottom of photo"
      >
        <Icon name="subtitles" size="12px" />
      </button>
      <button
        class="header-util-btn ghost"
        onclick={() => openManual(activeTab === "dev" && !developEngine ? "develop/engines/" : TAB_PAGES[activeTab])}
        title="Manual — about this tab"
      >
        <Icon name="question" size="12px" />
      </button>
      <button class="close header-util-btn ghost" onclick={() => hidePanel()} title="Close panel (⇧D)">
        <Icon name="x" size="11px" />
      </button>
    </header>

    <!-- TAB BAR -->
    <div class="tab-bar btn-group" role="tablist" aria-label="Develop panel">
      <button  role="tab" aria-selected={activeTab === 'dev'} onclick={() => activeTab = 'dev'} title="Dev" aria-label="Dev">
        <Icon name="sliders-horizontal" size="14px" />
      </button>
      <button  role="tab" aria-selected={activeTab === 'crop'} onclick={() => activeTab = 'crop'} title="Crop" aria-label="Crop">
        <Icon name="crop" size="14px" />
      </button>
      <button  role="tab" aria-selected={activeTab === 'preset'} onclick={() => activeTab = 'preset'} title="Presets" aria-label="Presets">
        <Icon name="stack-simple" size="14px" />
      </button>
      <button  role="tab" aria-selected={activeTab === 'info'} onclick={() => activeTab = 'info'} title="Editorial" aria-label="Editorial">
        <Icon name="newspaper" size="14px" />
      </button>
      <button  role="tab" aria-selected={activeTab === 'export'} onclick={() => activeTab = 'export'} title="Export" aria-label="Export">
        <Icon name="download-simple" size="14px" />
      </button>
    </div>
    <div class="hairline"></div>
  </div>

  <!-- SCROLLABLE TAB CONTENT -->
  <div class="pane-scroll">
    {#if activeTab === 'dev'}
      <DevTab
        bind:recipe
        {engines}
        {developEngine}
        {activeEngine}
        {films}
        {papers}
        {luts}
        {edited}
        {resetOne}
        {addLutLayer}
        {removeLutLayer}
        {updateLutOpacity}
        {setLutFile}
        {engineChanged}
        {resetRecipe}
        {onExport}
        {photoPath}
        {histogram}
        {scopes}
        bind:photoScale
        {onPhotoScaleChanged}
      />
    {:else if activeTab === 'crop'}
      <CropTab bind:recipe {edited} />
    {:else if activeTab === 'preset'}
      <PresetTab bind:recipe {engines} {photoPath} />
    {:else if activeTab === 'info'}
      <InfoBlock
        {picked}
        {exifLine}
        {exif}
        {renderMs}
        {status}
        {rating}
        bind:caption
        bind:tags
        {photoPath}
        {onCaptionEdited}
        {onTagsEdited}
      />
    {:else if activeTab === 'export'}
      <ExportTab
        {installedEditors}
        {exportFolder}
        bind:exportEdge
        bind:exportBorder
        {photoPath}
        bind:publishing
        bind:publishStatus
        {onExportSettingsChanged}
        {onExport}
        {onExportDaily}
        {onChooseExportFolder}
        {onOpenInEditor}
      />
    {/if}
  </div>
</div>

<style>
  .panel {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .sticky-top {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    z-index: 10;
    margin-block-end: var(--space-d2);
  }

  header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-d4);
    padding-block-end: var(--space-d2);
  }
  .close {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  header .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }
  .header-util-btn {
    cursor: pointer;
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
  }
  .clip-indicator {
    position: absolute;
    bottom: 2px;
    right: 2px;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-accent);
  }

  .hairline {
    height: 1px;
    background: var(--color-border);
    margin: 0 calc(var(--space-d4) * 3);
    flex-shrink: 0;
  }

  /* The tab bar is the framework's .btn-group — the same control as the
     engine switch and Frames / Editorial. This only places it. */
  .tab-bar {
    margin-block-end: var(--space-d2);
  }

  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    /* padding is handled within the individual tab components */

  }

</style>
