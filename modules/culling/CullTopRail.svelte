<script>
  import { invoke } from "@tauri-apps/api/core";
  import Dropdown from "@stnd/ui/Dropdown.svelte";
  import DropdownItem from "@stnd/ui/DropdownItem.svelte";
  import DropdownSeparator from "@stnd/ui/DropdownSeparator.svelte";
  import Popover from "@stnd/ui/Popover.svelte";
  import { activity, Photo, Icon } from "@modules/core";

  /**
   * @typedef {Object} Props
   * @property {boolean} [sidebarVisible]
   * @property {() => void} [onToggleSidebar]
   * @property {() => void} [onOpenSidebarPeek]
   * @property {() => void} [onScheduleSidebarPeekClose]
   * @property {boolean} [focusOn]
   * @property {() => void} [onToggleFocus]
   * @property {() => void} [onToggleAppearance]
   * @property {() => void} [onShowShortcuts]
   * @property {string | null} [curDir]
   * @property {string | null} [rootDir]
   * @property {(path: string) => string | undefined} [dirLabel]
   * @property {number} [minRating]
   * @property {(n: number) => void} [onSetMinRating]
   * @property {string} [textFilter]
   * @property {(text: string) => void} [onSetTextFilter]
   * @property {"all" | "picks" | "rejected" | "unflagged"} [pickFilter]
   * @property {(pick: "all" | "picks" | "rejected" | "unflagged") => void} [onSetPickFilter]
   * @property {boolean} [filterStory]
   * @property {() => void} [onToggleFilterStory]
   * @property {boolean} [sortDesc]
   * @property {(desc: boolean) => void} [onSetSortDesc]
   * @property {number} [viewLength]
   * @property {number} [framesLength]
   * @property {boolean} [applePhotosActive]
   * @property {number} [applePhotosTotal]
   * @property {number} [applePhotosOffset]
   * @property {boolean} [applePhotosBusy]
   * @property {() => void} [onLoadMoreApplePhotos]
   * @property {boolean} [immichActive]
   * @property {number} [immichTotal]
   * @property {number} [immichOffset]
   * @property {boolean} [immichBusy]
   * @property {() => void} [onLoadMoreImmich]
   * @property {boolean} [hasLibrary]
   * @property {boolean} [isTauri]
   * @property {boolean} [scanning]
   * @property {() => void} [onIndexRoot]
   * @property {any[]} [cards]
   * @property {any} [importingCard]
   * @property {(card: any) => void} [onImportCard]
   * @property {any} [ejectableCard]
   * @property {boolean} [ejecting]
   * @property {(card: any) => void} [onEjectCard]
   * @property {() => void} [onStopImport]
   * @property {() => void} [onStopCull]
   * @property {string | null} [publishedUrl]
   * @property {boolean} [hasPhotosToExport]
   * @property {number} [selectionCount]
   * @property {() => void} [onExportSelection]
   * @property {string} [layout]
   * @property {() => void} [onToggleLayout]
   * @property {number} [cols]
   * @property {(n: number) => void} [onSetCols]
   * @property {number} [cellAspect]
   * @property {(a: number) => void} [onSetCellAspect]
   * @property {boolean} [fillCells]
   * @property {() => void} [onToggleFillCells]
   * @property {number} [marginScale]
   * @property {(m: number) => void} [onSetMarginScale]
   * @property {number} [storySetSize]
   * @property {boolean} [gardenSignedIn]
   * @property {() => void} [onPublishStory]
   * @property {string} [publishVerb]
   * @property {boolean} [canCullFolder]
   * @property {() => void} [onCullFolder]
   * @property {boolean} [masonryTooBig]
   * @property {number} [masonryLimit]
   */

  /** @type {Props} */
  let {
    sidebarVisible = true,
    onToggleSidebar = () => {},
    onOpenSidebarPeek = () => {},
    onScheduleSidebarPeekClose = () => {},
    focusOn = false,
    onToggleFocus = () => {},
    onToggleAppearance = () => {},
    onShowShortcuts = () => {},
    curDir = "",
    rootDir = "",
    dirLabel = (p) => p,
    minRating = 0,
    onSetMinRating = () => {},
    textFilter = "",
    onSetTextFilter = () => {},
    pickFilter = "all",
    onSetPickFilter = () => {},
    filterStory = false,
    onToggleFilterStory = () => {},
    sortDesc = false,
    onSetSortDesc = () => {},
    viewLength = 0,
    framesLength = 0,
    applePhotosActive = false,
    applePhotosTotal = 0,
    applePhotosOffset = 0,
    applePhotosBusy = false,
    onLoadMoreApplePhotos = () => {},
    immichActive = false,
    immichTotal = 0,
    immichOffset = 0,
    immichBusy = false,
    onLoadMoreImmich = () => {},
    hasLibrary = false,
    isTauri = false,
    scanning = false,
    onIndexRoot = () => {},
    cards = [],
    importingCard = null,
    onImportCard = () => {},
    ejectableCard = null,
    ejecting = false,
    onEjectCard = () => {},
    onStopImport = () => {},
    onStopCull = () => {},
    publishedUrl = null,
    hasPhotosToExport = false,
    selectionCount = 0,
    onExportSelection = () => {},
    layout = "grid",
    onToggleLayout = () => {},
    cols = 4,
    onSetCols = () => {},
    cellAspect = 1,
    onSetCellAspect = () => {},
    fillCells = true,
    onToggleFillCells = () => {},
    marginScale = 1,
    onSetMarginScale = () => {},
    storySetSize = 0,
    gardenSignedIn = false,
    onPublishStory = () => {},
    publishVerb = "Publier",
    canCullFolder = false,
    onCullFolder = () => {},
    masonryTooBig = false,
    masonryLimit = 500,
  } = $props();

  let layoutMenuOpen = $state(false);

  /** @type {[string, number][]} */
  const aspects = [
    ["1:1", 1],
    ["4:5", 0.8],
    ["5:4", 1.25],
    ["3:2", 1.5],
    ["2:3", 0.667],
    ["16:9", 1.778],
  ];

  const pct = (/** @type {any} */ v, /** @type {number} */ min, /** @type {number} */ max) =>
    `${((Number(v ?? 0) - min) / (max - min)) * 100}%`;
</script>

<header class="rail" class:solo={!sidebarVisible} data-tauri-drag-region>
  {#if !sidebarVisible}
    <div class="brand-cluster">
      <button
        class="ghost icon small"
        onclick={onToggleSidebar}
        onmouseenter={onOpenSidebarPeek}
        onmouseleave={onScheduleSidebarPeekClose}
        title="Folder panel (B)"
      >
        <Icon name="sidebar-simple" size="12px" />
      </button>
      <button
        class="ghost icon small"
        aria-pressed={focusOn}
        onclick={onToggleFocus}
        title="Focus mode — dims the background (o)"
      >
        <span class="focus-glyph" class:on={focusOn}></span>
      </button>
      <button class="ghost icon small" onclick={onToggleAppearance} title="Toggle system light / dark mode (l)">
        <Icon name="circle-half" size="12px" />
      </button>
      <button class="wordmark" onclick={onShowShortcuts} title="Keyboard shortcuts">
        {curDir && curDir !== rootDir ? (dirLabel(curDir) ?? "").toUpperCase() : "REVEAL"}
      </button>
    </div>
  {/if}

  <!-- Filter (Rating, Pick, Story) -->
  <Dropdown label="Filter photos" triggerClass={`ghost ${minRating > 0 || filterStory || pickFilter !== "all" || textFilter ? "accent" : ""}`}>
    {#snippet trigger()}
      <Icon name="funnel" size="12px" />
      {#if minRating > 0}
        <span class="badge">{minRating}★</span>
      {:else if pickFilter === "picks"}
        <span class="badge">P</span>
      {:else if filterStory}
        <span class="badge">Q</span>
      {/if}
    {/snippet}
    <DropdownItem
      role="menuitemcheckbox"
      aria-checked={filterStory}
      onclick={onToggleFilterStory}
      disabled={applePhotosActive}
    >
      Story board only
      {#if filterStory}<Icon name="check" size="10px" />{/if}
    </DropdownItem>
    <DropdownSeparator />
    <DropdownItem role="menuitemradio" aria-checked={pickFilter === "all"} onclick={() => onSetPickFilter("all")}>
      All flags
      {#if pickFilter === "all"}<Icon name="check" size="10px" />{/if}
    </DropdownItem>
    <DropdownItem role="menuitemradio" aria-checked={pickFilter === "picks"} onclick={() => onSetPickFilter("picks")}>
      Picks only
      {#if pickFilter === "picks"}<Icon name="check" size="10px" />{/if}
    </DropdownItem>
    <DropdownItem role="menuitemradio" aria-checked={pickFilter === "unflagged"} onclick={() => onSetPickFilter("unflagged")}>
      Unflagged only
      {#if pickFilter === "unflagged"}<Icon name="check" size="10px" />{/if}
    </DropdownItem>
    <DropdownItem role="menuitemradio" aria-checked={pickFilter === "rejected"} onclick={() => onSetPickFilter("rejected")}>
      Rejected only
      {#if pickFilter === "rejected"}<Icon name="check" size="10px" />{/if}
    </DropdownItem>
    <DropdownSeparator />
    {#each [0, 1, 2, 3, 4, 5] as n}
      <DropdownItem role="menuitemradio" aria-checked={minRating === n} onclick={() => onSetMinRating(n)}>
        {n === 0 ? "Any rating" : `≥ ${n} ★`}
        {#if minRating === n}<Icon name="check" size="10px" />{/if}
      </DropdownItem>
    {/each}
  </Dropdown>

  <!-- Instant Search -->
  <div class="search-box">
    <Icon name="magnifying-glass" size="10px" class="search-ico" />
    <input
      type="text"
      class="search-input"
      placeholder="Search…"
      value={textFilter}
      oninput={(e) => onSetTextFilter(e.currentTarget.value)}
      aria-label="Filter photos by filename, caption or tags"
    />
    {#if textFilter}
      <button class="search-clear" onclick={() => onSetTextFilter("")} title="Clear filter">
        <Icon name="x" size="8px" />
      </button>
    {/if}
  </div>

  <!-- Sort -->
  <Dropdown label="Sort photos" triggerClass="ghost icon">
    {#snippet trigger()}
      <Icon name="arrows-down-up" size="12px" />
    {/snippet}
    <DropdownItem
      role="menuitemradio"
      aria-checked={!sortDesc}
      onclick={() => onSetSortDesc(false)}
    >
      Oldest first
      {#if !sortDesc}<Icon name="check" size="10px" />{/if}
    </DropdownItem>
    <DropdownItem
      role="menuitemradio"
      aria-checked={sortDesc}
      onclick={() => onSetSortDesc(true)}
    >
      Newest first
      {#if sortDesc}<Icon name="check" size="10px" />{/if}
    </DropdownItem>
  </Dropdown>

  <span class="frame-count titlebar-text">
    {#if viewLength !== framesLength}
      {viewLength}/{framesLength} FRAMES
    {:else}
      {viewLength} FRAMES
    {/if}
  </span>

  <span class="rail-spacer"></span>

  {#if applePhotosActive}
    <span class="frame-count titlebar-text">{framesLength} / {applePhotosTotal} Apple Photos</span>
    {#if applePhotosOffset < applePhotosTotal}
      <button class="rail-action" onclick={onLoadMoreApplePhotos} disabled={applePhotosBusy}>
        {applePhotosBusy ? "Loading..." : "Load more photos"}
      </button>
    {/if}
  {/if}

  {#if immichActive}
    <span class="frame-count titlebar-text">{framesLength} / {immichTotal} Immich</span>
    {#if immichOffset < immichTotal}
      <button class="rail-action" onclick={onLoadMoreImmich} disabled={immichBusy}>
        {immichBusy ? "Loading..." : "Load more photos"}
      </button>
    {/if}
  {/if}

  {#if !hasLibrary && !applePhotosActive}
    <button class="rail-action" onclick={onIndexRoot} disabled={!isTauri || scanning}>
      {scanning ? "indexing…" : "Index a library"}
    </button>
  {/if}

  <!-- Idle cards: one import button each. The card mid-import shows
       the live chip below instead, so hide its button. -->
  {#each cards as card (card.dcim)}
    {#if !importingCard || importingCard.dcim !== card.dcim}
      <button class="import rail-action" onclick={() => onImportCard(card)} disabled={!!activity.progress}>
        Import {card.name} ({card.raw_count})
      </button>
    {/if}
  {/each}

  <!-- A finished card import awaiting ejection — pull the card safely. -->
  {#if ejectableCard}
    <button
      class="rail-action"
      onclick={() => onEjectCard(ejectableCard)}
      disabled={ejecting}
    >
      {ejecting ? "Ejecting…" : `Eject ${ejectableCard.name}`}
    </button>
  {/if}

  <!-- The live import chip -->
  {#if activity.progress && activity.progress.verb === "import"}
    <span class="export-chip">
      {#if activity.progress.path}
        {#key activity.progress.path}
          <img class="chip-thumb" src={Photo.thumb(activity.progress.path)} alt="" />
        {/key}
      {/if}
      <span class="chip-label">Importing</span>
      <span class="chip-count">{activity.progress.done}/{activity.progress.total}</span>
      <progress class="chip-bar" value={activity.progress.done} max={activity.progress.total || 1}></progress>
      <button class="chip-stop" onclick={onStopImport} title="Stop the import">
        <Icon name="x" size="9px" />
      </button>
    </span>
  {/if}

  <!-- The live export chip -->
  {#if activity.progress && activity.progress.verb === "export"}
    <span class="export-chip">
      <span class="chip-label">Developing</span>
      <span class="chip-count">{activity.progress.done}/{activity.progress.total}</span>
      <progress class="chip-bar" value={activity.progress.done} max={activity.progress.total || 1}></progress>
    </span>
  {/if}

  <!-- The AI cull chip -->
  {#if activity.progress && activity.progress.verb === "cull"}
    <span class="export-chip">
      <span class="chip-label">AI Culling</span>
      <span class="chip-count">{activity.progress.done}/{activity.progress.total}</span>
      <progress class="chip-bar" value={activity.progress.done} max={activity.progress.total || 1}></progress>
      <button class="chip-stop" onclick={onStopCull} title="Stop the AI cull">
        <Icon name="x" size="9px" />
      </button>
    </span>
  {/if}

  {#if publishedUrl}
    <button
      class="ghost icon"
      onclick={() => invoke("open_path", { path: publishedUrl })}
      title="View the published note on Garden"
      aria-label="View the published note on Garden"
    >
      <Icon name="arrow-square-out" size="12px" />
    </button>
  {/if}

  {#if hasPhotosToExport}
    <button
      class="ghost icon"
      onclick={onExportSelection}
      disabled={!!activity.progress}
      title={selectionCount > 1 ? (selectionCount === viewLength ? `Export all photos (${viewLength}) (r)` : `Export the ${selectionCount} selected photos (r)`) : `Export the selected photo (r)`}
    >
      <Icon name="export" size="12px" />
    </button>
  {/if}

  <!-- The grid's whole geometry behind ONE icon. -->
  <Popover bind:open={layoutMenuOpen} label="Grid layout" align="end">
    {#snippet trigger(/** @type {import('svelte/elements').HTMLButtonAttributes} */ attributes)}
      <button
        class="ghost icon"
        {...attributes}
        aria-label="Grid layout"
        title="Grid layout"
      >
        <Icon name={layout === "masonry" ? "rows" : "grid-four"} size="12px" />
      </button>
    {/snippet}
    <div class="layout-controls">
      {#if publishedUrl}
        <button
          class="std-menu-item"
          onclick={() => {
            layoutMenuOpen = false;
            if (publishedUrl) invoke("open_path", { path: publishedUrl });
          }}
        >
          <span class="item-label">Open on the web</span>
          <Icon name="arrow-square-out" size="10px" />
        </button>
      {/if}
      {#if storySetSize && gardenSignedIn}
        <button
          class="std-menu-item"
          class:disabled={!!activity.progress}
          onclick={() => {
            layoutMenuOpen = false;
            onPublishStory();
          }}
          disabled={!!activity.progress}
        >
          <span class="item-label">{publishVerb} l'histoire ({storySetSize})</span>
          <Icon name="lightning" size="10px" />
        </button>
      {/if}
      {#if canCullFolder}
        <button
          class="std-menu-item"
          class:disabled={!!activity.progress}
          onclick={() => {
            layoutMenuOpen = false;
            onCullFolder();
          }}
          disabled={!!activity.progress}
        >
          <span class="item-label">AI Culling</span>
          <Icon name="lightning" size="10px" />
        </button>
      {/if}
      {#if publishedUrl || storySetSize || canCullFolder}
        <div class="std-menu-separator"></div>
      {/if}
      <span class="pop-label">Columns</span>
      <div class="pop-grid">
        {#each [1, 2, 3, 4, 5, 6, 8, 10, 12] as n}
          <button
            aria-pressed={cols === n}
            onclick={() => onSetCols(n)}
          >{n}</button>
        {/each}
      </div>
      <div class="std-menu-separator"></div>
      <span class="pop-label">Format</span>
      <button
        class="std-menu-item"
        disabled={masonryTooBig}
        title={masonryTooBig
          ? `Masonry draws every photo at once; ${framesLength} is past what stays smooth (limit ${masonryLimit})`
          : undefined}
        onclick={onToggleLayout}
      >
        <span class="item-label">Masonry</span>
        {#if masonryTooBig}
          <span class="item-note">{framesLength} &gt; {masonryLimit}</span>
        {:else if layout === "masonry"}
          <Icon name="check" size="10px" />
        {/if}
      </button>
      {#if layout !== "masonry"}
        <div class="pop-grid three">
          {#each aspects as [label, a]}
            <button
              aria-pressed={Math.abs(cellAspect - a) < 0.001}
              onclick={() => onSetCellAspect(a)}
            >{label}</button>
          {/each}
        </div>
        <button
          class="std-menu-item"
          onclick={onToggleFillCells}
        >
          <span class="item-label">{fillCells ? "Fill cells" : "Keep aspect ratio"}</span>
          {#if !fillCells}<Icon name="check" size="10px" />{/if}
        </button>
      {/if}
      <div class="std-menu-separator"></div>
      <span class="pop-label">Marge</span>
      <input
        class="pop-slider"
        type="range"
        min="0.25"
        max="6"
        step="0.25"
        value={marginScale}
        aria-label="Grid margin"
        style="--slider-value: {pct(marginScale, 0.25, 6)}"
        oninput={(e) => onSetMarginScale(Number(/** @type {HTMLInputElement} */ (e.currentTarget).value))}
      />
    </div>
  </Popover>
</header>

<style>
  .rail {
    height: var(--titlebar-height);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    flex-wrap: nowrap;
    white-space: nowrap;
    gap: calc(var(--space-d4) * 3);
    padding: 0 var(--space);
    position: relative;
    z-index: 20;
  }
  .rail.solo {
    padding-left: var(--window-controls-offset-content, 86px);
  }
  .rail :global(button) {
    white-space: nowrap;
    flex-shrink: 0;
  }
  .brand-cluster {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d2);
    margin-right: var(--space-d4);
  }
  .focus-glyph {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: var(--stroke-width) solid currentColor;
    position: relative;
  }
  .focus-glyph::after {
    content: "";
    position: absolute;
    inset: 2px;
    border-radius: 50%;
    border: var(--stroke-width) solid currentColor;
  }
  .focus-glyph.on::after {
    background: currentColor;
  }
  .wordmark {
    cursor: pointer;
  }
  .frame-count {
    flex-shrink: 0;
  }
  .rail-spacer {
    flex: 1;
    min-width: 12px;
  }
  .rail-action {
    box-sizing: border-box;
    cursor: pointer;
    flex-shrink: 0;
  }
  .rail-action:disabled {
    cursor: default;
  }
  .layout-controls {
    min-width: 208px;
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }
  .std-menu-item[disabled] {
    opacity: 0.45;
    cursor: default;
    pointer-events: auto;
  }
  .item-note {
    white-space: nowrap;
  }
  .pop-label {
    padding: 0 var(--space-d2);
  }
  .pop-grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: var(--space-d8);
    padding: 0 var(--space-d4);
  }
  .pop-grid.three {
    grid-template-columns: repeat(3, 1fr);
  }
  .pop-slider {
    width: auto;
    margin: 0 var(--space-d2) var(--space-d4);
  }
  .export-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d3);
    flex-shrink: 0;
  }
  .chip-bar {
    width: 40px;
    flex-shrink: 0;
  }
  .chip-thumb {
    width: 20px;
    height: 20px;
    object-fit: cover;
    border-radius: var(--radius);
    flex-shrink: 0;
    box-shadow: 0 0 0 var(--stroke-width) var(--color-border);
  }
  .chip-stop {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
  }
  .search-box {
    display: inline-flex;
    align-items: center;
    position: relative;
    background: var(--color-surface);
    border-radius: var(--radius-sm);
    padding: 0 var(--space-d4);
    height: 20px;
    width: 120px;
    transition: width var(--duration-fast, 0.15s);
  }
  .search-box:focus-within {
    width: 160px;
    box-shadow: 0 0 0 1px var(--color-accent, var(--color-foreground));
  }
  :global(.search-ico) {
    color: var(--color-muted);
    opacity: 0.7;
    margin-right: var(--space-d4);
    flex-shrink: 0;
  }
  .search-input {
    all: unset;
    font-size: 10px;
    width: 100%;
    color: var(--color-foreground);
  }
  .search-input::placeholder {
    color: var(--color-muted);
    opacity: 0.6;
  }
  .search-clear {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-muted);
    padding: 2px;
  }
  .search-clear:hover {
    color: var(--color-foreground);
  }
</style>
