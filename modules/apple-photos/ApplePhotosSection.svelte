<script>
  import { Icon } from "@modules/core";
  import { APPLE_PHOTOS_ROOT, photoCollectionAncestors } from "./applePhotosTree.js";
  import ApplePhotosRow from "./ApplePhotosRow.svelte";

  /**
   * @typedef {import('./applePhotosTree.js').PhotoLibrary} PhotoLibrary
   */

  /**
   * @typedef {Object} Props
   * @property {PhotoLibrary} applePhotos
   * @property {boolean} [open]
   * @property {() => void} [onToggle]
   * @property {(path: string) => void} onOpenDir
   * @property {() => void} [onConnect]
   * @property {() => void} [onRefresh]
   */

  /** @type {Props} */
  let {
    applePhotos,
    open = false,
    onToggle = () => {},
    onOpenDir,
    onConnect = () => {},
    onRefresh = () => {},
  } = $props();

  let expandedIds = $state(new Set());
  let manuallyCollapsedIds = $state(new Set());

  const ancestors = $derived(new Set(applePhotos?.active
    ? photoCollectionAncestors(applePhotos.albums, applePhotos.album)
        .map((a) => a.replace(APPLE_PHOTOS_ROOT, ""))
        .slice(0, -1)
    : []));

  /** @param {string} id */
  function isCollectionExpanded(id) {
    if (manuallyCollapsedIds.has(id)) return false;
    if (expandedIds.has(id)) return true;
    return ancestors.has(id);
  }

  /** @param {string} id */
  function toggleCollection(id) {
    if (isCollectionExpanded(id)) {
      const nextExp = new Set(expandedIds);
      nextExp.delete(id);
      expandedIds = nextExp;
      if (ancestors.has(id)) {
        const nextCol = new Set(manuallyCollapsedIds);
        nextCol.add(id);
        manuallyCollapsedIds = nextCol;
      }
    } else {
      const nextCol = new Set(manuallyCollapsedIds);
      nextCol.delete(id);
      manuallyCollapsedIds = nextCol;
      const nextExp = new Set(expandedIds);
      nextExp.add(id);
      expandedIds = nextExp;
    }
  }

  const isCurrentAll = $derived(!!applePhotos?.active && !applePhotos.album);
  const totalLabel = $derived(applePhotos?.loaded ? (applePhotos.total ?? 0).toLocaleString("en-CA") : "");

  function handleToggle() {
    onToggle();
    if (!open && !applePhotos?.loaded) {
      onConnect();
    }
  }

  function handleOpenAll() {
    if (!open) {
      onToggle();
      if (!applePhotos?.loaded) onConnect();
    }
    onOpenDir(APPLE_PHOTOS_ROOT);
  }

  /** @param {string} id */
  function handleNavigate(id) {
    onOpenDir(APPLE_PHOTOS_ROOT + id);
  }

  /** @param {string} id */
  function isCurrent(id) {
    return !!applePhotos?.active && applePhotos.album === id;
  }
</script>

<div class="section">
  <button
    class="cat-disc"
    onclick={(e) => {
      e.stopPropagation();
      handleToggle();
    }}
    disabled={applePhotos?.busy && !applePhotos?.loaded}
    aria-label={`${open ? "Collapse" : "Expand"} Apple Photos`}
    aria-expanded={open}
    title={open ? "Collapse catalogue" : "Expand catalogue"}
  >
    <span class="disc" class:open><Icon name="caret-right" size="var(--icon-sm)" /></span>
  </button>
  <button
    class="section-main ghost"
    title="Apple Photos"
    aria-current={isCurrentAll ? "true" : undefined}
    disabled={applePhotos?.busy}
    onclick={handleOpenAll}
    oncontextmenu={(e) => e.preventDefault()}
  >
    <span class="section-name">Apple Photos</span>
  </button>
  <span class="dir-spacer"></span>
  <span class="dir-count">{totalLabel}</span>
  <button
    class="add-btn"
    onclick={(e) => {
      e.stopPropagation();
      onRefresh();
    }}
    disabled={applePhotos?.busy}
    title="Refresh Apple Photos"
    aria-label="Refresh Apple Photos"
    aria-busy={applePhotos?.busy}
  >
    <Icon name="arrows-clockwise" size="var(--icon-sm)" class={applePhotos?.busy ? "spin" : ""} />
  </button>
</div>

{#if open && applePhotos?.albums}
  {#each applePhotos.albums as collection (collection.id)}
    <ApplePhotosRow
      {collection}
      depth={0}
      {isCurrent}
      isExpanded={isCollectionExpanded}
      onToggle={toggleCollection}
      onNavigate={handleNavigate}
      busy={Boolean(applePhotos?.busy)}
    />
  {/each}
{/if}

<style>
  .section {
    display: flex;
    align-items: center;
    padding: var(--space-d4) var(--space-d2);
    margin-top: var(--space-d3);
    box-shadow: var(--shadow-border-top);
    user-select: none;
    font-size: 11px;
    font-weight: 600;
  }
  .cat-disc {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
  }
  .cat-disc .disc {
    display: flex;
    align-items: center;
    justify-content: center;
    transition: transform var(--duration-fast);
  }
  .cat-disc .disc.open {
    transform: rotate(90deg);
  }
  .section-main {
    all: unset;
    cursor: pointer;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 2px 4px;
    border-radius: var(--radius-sm);
  }
  .section-main:hover {
    background: var(--color-hover);
  }
  .section-name {
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-muted);
  }
  .dir-spacer {
    flex: 1;
  }
  .dir-count {
    font-variant-numeric: tabular-nums;
    color: var(--color-muted);
    font-size: 10px;
    margin-right: var(--space-d4);
  }
  .add-btn {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: var(--radius-sm);
    color: var(--color-muted);
  }
  .add-btn:hover {
    color: var(--color-foreground);
    background: var(--color-hover);
  }
  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
