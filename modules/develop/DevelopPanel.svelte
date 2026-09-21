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
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";
  import InfoBlock from "./tabs/InfoBlock.svelte";
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
    // The Photo Size slider's own value — bindable for instant local echo
    // (mirrors exportEdge/exportBorder's pattern), onPhotoScaleChanged is
    // what actually makes it stick: a no-op when docked (this panel shares
    // memory with the window that owns the photo), a relay emit when
    // detached (see routes/dev-panel/+page.svelte).
    photoScale = $bindable(90),
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
        class="header-util-btn"
        onclick={() => onToggleDetached()}
        title={detached ? "Re-dock the panel" : "Detach into its own window"}
      >
        <Icon name={detached ? "arrows-in-simple" : "arrow-square-out"} size="12px" />
      </button>
      <button
        class="header-util-btn"
        class:active={showClipping}
        onclick={() => toggleClipping()}
        title="Clipping warning (highlights & shadows)"
      >
        <Icon name="circle-half" size="12px" />
        {#if showClipping}
          <span class="clip-indicator"></span>
        {/if}
      </button>
      <button
        class="header-util-btn"
        class:active={showCaption}
        onclick={() => toggleCaptionOverlay()}
        title="Show caption at bottom of photo"
      >
        <Icon name="subtitles" size="12px" />
      </button>
      <button class="close" onclick={() => hidePanel()} title="Close panel (⇧D)">
        <Icon name="x" size="11px" />
      </button>
    </header>

    <!-- TAB BAR -->
    <div class="tab-bar">
      <button class="tab-btn" class:active={activeTab === 'dev'} onclick={() => activeTab = 'dev'} title="Dev" aria-label="Dev">
        <Icon name="sliders-horizontal" size="14px" />
      </button>
      <button class="tab-btn" class:active={activeTab === 'crop'} onclick={() => activeTab = 'crop'} title="Crop" aria-label="Crop">
        <Icon name="crop" size="14px" />
      </button>
      <button class="tab-btn" class:active={activeTab === 'preset'} onclick={() => activeTab = 'preset'} title="Presets" aria-label="Presets">
        <Icon name="stack-simple" size="14px" />
      </button>
      <button class="tab-btn" class:active={activeTab === 'info'} onclick={() => activeTab = 'info'} title="Editorial" aria-label="Editorial">
        <Icon name="newspaper" size="14px" />
      </button>
      <button class="tab-btn" class:active={activeTab === 'export'} onclick={() => activeTab = 'export'} title="Export" aria-label="Export">
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
        bind:photoScale
        {onPhotoScaleChanged}
      />
    {:else if activeTab === 'crop'}
      <CropTab bind:recipe {edited} />
    {:else if activeTab === 'preset'}
      <PresetTab bind:recipe {engines} />
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
    background: var(--color-surface-high);
  }

  .sticky-top {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-surface-high);
    z-index: 10;
  }

  .din {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }

  header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px 10px;
  }
  .close {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    transition: color var(--duration-fast), background var(--duration-fast);
  }
  .close:hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 10%, transparent);
  }
  header .title {
    color: var(--color-foreground);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }
  .header-util-btn {
    all: unset;
    cursor: pointer;
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm, 4px);
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    transition: color var(--duration-fast), background var(--duration-fast);
  }
  .header-util-btn:hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }
  .header-util-btn.active {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
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
    margin: 0 12px;
    flex-shrink: 0;
  }

  .tab-bar {
    display: flex;
    gap: 3px;
    margin: 0 12px 8px;
    padding: 2px;
    background: color-mix(in srgb, var(--color-foreground) 3.5%, transparent);
    border-radius: var(--radius-sm, 4px);
  }
  /* Height is 1 line of the app's own text (font-size × line-height), not
     padding — the same vertical-rhythm unit every other compact control in
     this dev panel now shares (the segmented engine switch's .seg-btn, in
     DevTab.svelte), so a row of icon buttons and a row of text pills read
     as the same size instead of each accumulating its own padding-derived
     height. calc() off the app's own tokens rather than the 1rlh unit —
     this WebView doesn't appear to support rlh, so it silently fell back
     to intrinsic content sizing (~26px) instead of the intended ~16px. */
  .tab-btn {
    all: unset;
    cursor: pointer;
    flex: 1;
    height: calc(var(--font-text-size) * var(--line-height));
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    transition: color var(--duration-fast) var(--ease-soft), background var(--duration-fast) var(--ease-soft), box-shadow var(--duration-fast) var(--ease-soft);
  }
  .tab-btn:hover {
    color: var(--color-foreground);
  }
  .tab-btn.active {
    color: var(--color-foreground);
    background: var(--color-surface-high);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  }

  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    /* padding is handled within the individual tab components */
  }
</style>
