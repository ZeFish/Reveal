<script>
  import { Icon, Source, Collection, notify, virtualCollections } from "@modules/core";
  import {
    buildCatalogueTrees,
    filterGhosts,
    readPersistedSet,
    writePersistedSet,
    EXPANDED_KEY,
    MANUALLY_COLLAPSED_KEY,
    CAT_EXPANDED_KEY,
  } from "./treeOperations.js";
  import { APPLE_PHOTOS_ROOT, ApplePhotosSection } from "@modules/apple-photos";
  import { IMMICH_ROOT, ImmichSection } from "@modules/immich";

  // Subcomponents
  import CatalogueSection from "./CatalogueSection.svelte";
  import CatalogueTreeRow from "./CatalogueTreeRow.svelte";

  /** @typedef {import('./treeOperations.js').TreeNode} TreeNode */

  /**
   * @typedef {Object} Props
   * @property {string | null} [root]
   * @property {string[]} [roots]
   * @property {any[]} dirs
   * @property {string | null} curDir
   * @property {Array<import('@modules/core').Collection>} [collections]
   * @property {boolean} [scanning]
   * @property {{dirs: number, frames: number} | null} [indexProgress]
   * @property {boolean} [isLibrary]
   * @property {Set<string>} [storyDirs]
   * @property {string | null} [importDir]
   * @property {import('@modules/apple-photos/applePhotosTree.js').PhotoLibrary | null} [applePhotos]
   * @property {{connected: boolean, active: boolean, busy: boolean, loaded: boolean, album: string, albums: Array<{id: string, title: string, count: number}>, total: number | null} | null} [immich]
   * @property {string | null} [renamingPath]
   * @property {string} [renameValue]
   * @property {string | null} [creatingIn]
   * @property {string} [createValue]
   * @property {(path: string) => void} onOpenDir
   * @property {() => void} onOpenLibrary
   * @property {() => void} onAddLocation
   * @property {() => void} [onConnectApplePhotos]
   * @property {() => void} [onRefreshApplePhotos]
   * @property {() => void} [onConnectImmich]
   * @property {() => void} [onRefreshImmich]
   * @property {(path: string) => void} [onRescanDir]
   * @property {(paths: string[], destAbs: string) => void} [onMovePhotos]
   * @property {(srcAbs: string, destAbs: string) => Promise<void> | void} [onMoveDir]
   * @property {(path: string, newName: string) => Promise<void> | void} [onRenameDir]
   * @property {(parentAbs: string, name: string) => Promise<string | null | void> | string | null | void} [onCreateFolder]
   * @property {(event: MouseEvent, path: string, label: string) => void} onOpenFolderMenu
   */

  /** @type {Props} */
  let {
    root = null,
    roots = [],
    dirs = [],
    curDir = null,
    collections = [],
    scanning = false,
    indexProgress = null,
    isLibrary = false,
    storyDirs = new Set(),
    importDir = null,
    applePhotos = null,
    immich = null,
    renamingPath = $bindable(null),
    renameValue = $bindable(""),
    creatingIn = $bindable(null),
    createValue = $bindable("New Folder"),
    onOpenDir,
    onOpenLibrary,
    onAddLocation,
    onConnectApplePhotos = () => {},
    onRefreshApplePhotos = () => {},
    onConnectImmich = () => {},
    onRefreshImmich = () => {},
    onRescanDir = () => {},
    onMovePhotos = () => {},
    onMoveDir = () => {},
    onRenameDir = () => {},
    onCreateFolder = () => {},
    onOpenFolderMenu,
  } = $props();

  let expanded = $state(readPersistedSet(EXPANDED_KEY));
  let manuallyCollapsed = $state(readPersistedSet(MANUALLY_COLLAPSED_KEY));
  let expandedCatalogs = $state(readPersistedSet(CAT_EXPANDED_KEY));

  /** @type {string | null} */
  let dropTarget = $state(null);
  /** @type {string | null} */
  let draggingFolder = $state(null);

  /** @type {{ abs: string, name: string, parentAbs: string }[]} */
  let ghosts = $state([]);

  /** @param {string} rel */
  function isAncestor(rel) {
    return !!curDir?.startsWith(rel + "/");
  }

  /** @param {string} rel */
  function isExpanded(rel) {
    if (manuallyCollapsed.has(rel)) return false;
    if (expanded.has(rel)) return true;
    return isAncestor(rel);
  }

  /** @param {string} cat */
  function isCatExpanded(cat) {
    return !expandedCatalogs.has(`collapsed:${cat}`);
  }

  /** @param {string} rel */
  function toggle(rel) {
    if (isExpanded(rel)) {
      const nextExpanded = new Set(expanded);
      nextExpanded.delete(rel);
      expanded = nextExpanded;
      writePersistedSet(EXPANDED_KEY, nextExpanded);
      if (isAncestor(rel)) {
        const nextCollapsed = new Set(manuallyCollapsed);
        nextCollapsed.add(rel);
        manuallyCollapsed = nextCollapsed;
        writePersistedSet(MANUALLY_COLLAPSED_KEY, nextCollapsed);
      }
    } else {
      const nextCollapsed = new Set(manuallyCollapsed);
      nextCollapsed.delete(rel);
      manuallyCollapsed = nextCollapsed;
      const nextExpanded = new Set(expanded);
      nextExpanded.add(rel);
      expanded = nextExpanded;
      writePersistedSet(MANUALLY_COLLAPSED_KEY, nextCollapsed);
      writePersistedSet(EXPANDED_KEY, nextExpanded);
    }
  }

  /** @param {string} cat */
  function toggleCatExpanded(cat) {
    const next = new Set(expandedCatalogs);
    const key = `collapsed:${cat}`;
    if (isCatExpanded(cat)) next.add(key);
    else next.delete(key);
    expandedCatalogs = next;
    writePersistedSet(CAT_EXPANDED_KEY, next);
  }

  const catalogueTrees = $derived.by(() =>
    buildCatalogueTrees({
      roots,
      root,
      dirs,
      isCatExpanded,
    }),
  );

  const grandTotal = $derived(dirs.reduce((sum, d) => sum + d.count, 0));

  /** @param {TreeNode} node */
  function isCurrent(node) {
    return !isLibrary && node.abs === curDir;
  }

  /** @param {string} abs */
  function isImportDest(abs) {
    return !!importDir && importDir === abs;
  }
  /** @param {string} abs */
  function isImportBranch(abs) {
    return !!importDir && (importDir === abs || importDir.startsWith(abs + "/"));
  }

  /** @param {string|null} rel */
  function ensureExpanded(rel) {
    if (!rel) return;
    if (manuallyCollapsed.has(rel)) {
      const nextCollapsed = new Set(manuallyCollapsed);
      nextCollapsed.delete(rel);
      manuallyCollapsed = nextCollapsed;
      writePersistedSet(MANUALLY_COLLAPSED_KEY, nextCollapsed);
    }
    if (expanded.has(rel)) return;
    const next = new Set(expanded);
    next.add(rel);
    expanded = next;
    writePersistedSet(EXPANDED_KEY, next);
  }

  $effect(() => {
    ghosts = filterGhosts(ghosts, dirs);
  });

  $effect(() => {
    if (creatingIn) ensureExpanded(creatingIn);
  });

  /** @param {TreeNode} node */
  function navigate(node) {
    if (node.children.length) ensureExpanded(node.rel);
    onOpenDir(node.abs);
  }

  const PHOTO_MIME = "application/x-reveal-photos";
  const FOLDER_MIME = "application/x-reveal-folder";

  /** @param {DragEvent} event */
  function canDropPhotos(event) {
    if (!event.dataTransfer) return false;
    const types = Array.from(event.dataTransfer.types || []);
    return types.includes(PHOTO_MIME) || types.includes("text/plain") || types.includes("Files");
  }

  /** @param {DragEvent} event */
  function canDropFolder(event) {
    return !!event.dataTransfer && [...event.dataTransfer.types].includes(FOLDER_MIME);
  }

  /**
   * @param {DragEvent} event
   * @param {string} abs
   */
  function onRowDragOver(event, abs) {
    if (!canDropPhotos(event) && !canDropFolder(event)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    dropTarget = abs;
  }

  /** @param {string} abs */
  function onRowDragLeave(abs) {
    if (dropTarget === abs) dropTarget = null;
  }

  /**
   * @param {DragEvent} event
   * @param {string} abs
   */
  function onRowDrop(event, abs) {
    if (canDropFolder(event)) {
      onFolderRowDrop(event, abs);
      return;
    }
    if (!canDropPhotos(event)) return;
    event.preventDefault();
    dropTarget = null;
    let paths = [];
    try {
      const raw = event.dataTransfer?.getData(PHOTO_MIME) || event.dataTransfer?.getData("text/plain") || "";
      if (raw.startsWith("[")) {
        paths = JSON.parse(raw);
      } else if (raw) {
        paths = [raw];
      }
    } catch (_) {}
    if (paths.length) onMovePhotos(paths, abs);
  }

  /**
   * @param {DragEvent} event
   * @param {Collection} col
   */
  function onVirtualRowDrop(event, col) {
    if (!canDropPhotos(event)) return;
    event.preventDefault();
    dropTarget = null;
    let paths = [];
    try {
      const raw = event.dataTransfer?.getData(PHOTO_MIME) || event.dataTransfer?.getData("text/plain") || "";
      if (raw.startsWith("[")) {
        paths = JSON.parse(raw);
      } else if (raw) {
        paths = [raw];
      }
    } catch (_) {}
    if (!paths.length) return;

    if (col.id.startsWith("virtual://curated")) {
      const added = virtualCollections.addPhotos(col.id, paths);
      notify(`${added} photo${added > 1 ? "s" : ""} ajoutée${added > 1 ? "s" : ""} à ${col.name} ✓`);
    } else {
      notify(`Collection dynamique : filtre automatique.`);
    }
  }

  /**
   * @param {DragEvent} event
   * @param {TreeNode} node
   */
  function onFolderDragStart(event, node) {
    event.stopPropagation();
    draggingFolder = node.abs;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData(FOLDER_MIME, JSON.stringify({ path: node.abs, name: node.name }));
    }
  }

  function onFolderDragEnd() {
    draggingFolder = null;
  }

  /**
   * @param {DragEvent} event
   * @param {string} destAbs
   */
  async function onFolderRowDrop(event, destAbs) {
    event.preventDefault();
    dropTarget = null;
    let payload;
    try {
      payload = JSON.parse(event.dataTransfer?.getData(FOLDER_MIME) ?? "");
    } catch (_) {
      return;
    }
    draggingFolder = null;
    if (!payload?.path || payload.path === destAbs) return;
    if (destAbs.startsWith(payload.path + "/")) return;
    await onMoveDir(payload.path, destAbs);
  }

  async function commitRename() {
    const path = renamingPath;
    const value = renameValue.trim();
    renamingPath = null;
    if (!path || !value) return;
    await onRenameDir(path, value);
  }

  async function commitCreateFolder() {
    const parentAbs = creatingIn;
    const name = createValue.trim();
    creatingIn = null;
    if (!parentAbs || !name) return;
    try {
      const abs = await onCreateFolder(parentAbs, name);
      if (abs) ghosts = [...ghosts, { abs, name, parentAbs }];
    } catch (_) {}
  }

  /** @param {string} parentAbs */
  function ghostsUnder(parentAbs) {
    return ghosts.filter((g) => g.parentAbs === parentAbs);
  }

  /** @param {TreeNode} node */
  function hasChildren(node) {
    return node.children.length > 0 || ghostsUnder(node.abs).length > 0;
  }

  /** @param {HTMLInputElement} node */
  const selectOnFocus = (node) => {
    node.focus();
    node.select();
  };
</script>

<div class="tree">
  <!-- The index-wide view — the base of everything. -->
  <div
    class="item lib-row"
    aria-current={isLibrary ? "true" : undefined}
    class:is-drop-target={root && dropTarget === root}
    role="button"
    tabindex="0"
    aria-disabled={!root}
    onclick={() => { if (root) onOpenLibrary(); }}
    oncontextmenu={(event) => root && onOpenFolderMenu(event, root, "Library")}
    ondragover={(event) => root && onRowDragOver(event, root)}
    ondragleave={() => root && onRowDragLeave(root)}
    ondrop={(event) => root && onRowDrop(event, root)}
    onkeydown={(e) => {
      if (e.key === "Enter" && root && e.target === e.currentTarget) onOpenLibrary();
    }}
  >
    <span class="lead" aria-hidden="true"></span>
    <span class="lib-label">All Library</span>
    <span class="dir-spacer"></span>
    {#if scanning}
      <span class="indexing" title="Indexing">
        <Icon name="arrows-clockwise" size="9px" class="spin" />
        <span class="idx-count">{(indexProgress?.frames ?? grandTotal).toLocaleString("en-CA")}</span>
      </span>
    {:else}
      <span class="dir-count">{grandTotal.toLocaleString("en-CA")}</span>
    {/if}
    <button
      class="add-btn"
      onclick={(e) => {
        e.stopPropagation();
        onAddLocation();
      }}
      title="Add catalogue"
    >
      <Icon name="plus" size="9px" />
    </button>
  </div>

  <!-- Virtual & Curated Collections -->
  {#if collections?.length}
    <div class="virtual-section">
      <div class="section-title-row">
        <span class="section-title">Collections</span>
        <button
          class="add-btn"
          onclick={(e) => {
            e.stopPropagation();
            const name = prompt("Nom de la collection :", "Nouvelle collection");
            if (name?.trim()) {
              const created = virtualCollections.create(name.trim());
              notify(`Collection "${created.name}" créée ✓`);
            }
          }}
          title="Créer une collection"
        >
          <Icon name="plus" size="9px" />
        </button>
      </div>
      {#each collections as col (col.id)}
        <div
          class="item dir-row virtual-row"
          class:is-drop-target={dropTarget === col.id}
          aria-current={curDir === col.id ? "true" : undefined}
          role="button"
          tabindex="0"
          onclick={() => onOpenDir(col.id)}
          ondragover={(event) => onRowDragOver(event, col.id)}
          ondragleave={() => onRowDragLeave(col.id)}
          ondrop={(event) => onVirtualRowDrop(event, col)}
          oncontextmenu={(e) => {
            if (col.id.startsWith("virtual://curated")) {
              e.preventDefault();
              e.stopPropagation();
              if (confirm(`Supprimer la collection "${col.name}" ?`)) {
                virtualCollections.remove(col.id);
                notify(`Collection "${col.name}" supprimée`);
              }
            }
          }}
          onkeydown={(e) => {
            if (e.key === "Enter") onOpenDir(col.id);
          }}
        >
          <span class="disc"><Icon name={col.icon || "bookmark"} size="11px" /></span>
          <span class="dir-name">{col.name}</span>
          <span class="dir-spacer"></span>
          {#if col.count > 0}
            <span class="dir-count">{col.count.toLocaleString("en-CA")}</span>
          {/if}
          {#if col.id.startsWith("virtual://curated")}
            <button
              class="del-col-btn"
              onclick={(e) => {
                e.stopPropagation();
                if (confirm(`Supprimer la collection "${col.name}" ?`)) {
                  virtualCollections.remove(col.id);
                  notify(`Collection "${col.name}" supprimée`);
                }
              }}
              title="Supprimer la collection"
            >
              <Icon name="x" size="8px" />
            </button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <!-- Section per catalogue with collapse toggle & open handling -->
  {#each catalogueTrees as { cat, name, total, tree } (cat)}
    {@const open = isCatExpanded(cat)}
    {@const source = Source.fromLocalFolder(cat, total)}
    <CatalogueSection
      {cat}
      {name}
      {total}
      {source}
      {open}
      isDest={isImportDest(cat)}
      isBranch={isImportBranch(cat)}
      isCurrent={curDir === cat}
      {scanning}
      onToggle={() => toggleCatExpanded(cat)}
      onOpen={() => {
        if (!isCatExpanded(cat)) toggleCatExpanded(cat);
        onOpenDir(cat);
      }}
      onRefresh={() => onRescanDir(cat)}
      onContextMenu={(event) => onOpenFolderMenu(event, cat, name)}
    />

    {#if open}
      {#each tree.nodes as node (node.rel)}
        <CatalogueTreeRow
          {node}
          depth={0}
          {isCurrent}
          {isExpanded}
          {isImportDest}
          {isImportBranch}
          {hasChildren}
          {ghostsUnder}
          {dropTarget}
          {draggingFolder}
          {renamingPath}
          bind:renameValue
          {creatingIn}
          bind:createValue
          {storyDirs}
          onNavigate={navigate}
          onToggle={toggle}
          {onOpenFolderMenu}
          {onFolderDragStart}
          {onFolderDragEnd}
          {onRowDragOver}
          {onRowDragLeave}
          {onRowDrop}
          onCommitRename={commitRename}
          onCancelRename={() => { renamingPath = null; }}
          onCommitCreateFolder={commitCreateFolder}
          onCancelCreateFolder={() => { creatingIn = null; }}
          {onOpenDir}
        />
      {/each}
      {#each ghostsUnder(cat) as g (g.abs)}
        <div
          class="item dir-row"
          aria-current={curDir === g.abs ? "true" : undefined}
          class:is-drop-target={dropTarget === g.abs}
          style="--depth: 0"
          role="button"
          tabindex="0"
          onclick={() => onOpenDir(g.abs)}
          ondragover={(event) => onRowDragOver(event, g.abs)}
          ondragleave={() => onRowDragLeave(g.abs)}
          ondrop={(event) => onRowDrop(event, g.abs)}
          onkeydown={(e) => {
            if (e.key === "Enter") onOpenDir(g.abs);
          }}
        >
          <span class="disc"></span>
          <span class="dir-name" title={g.abs}>{g.name}</span>
          <span class="dir-spacer"></span>
          <span class="dir-count"></span>
          <span class="story-slot"></span>
        </div>
      {/each}
      {#if creatingIn === cat}
        <div class="dir-row" style="--depth: 0">
          <span class="disc"></span>
          <input
            class="dir-rename"
            value={createValue}
            oninput={(e) => (createValue = e.currentTarget.value)}
            onkeydown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") commitCreateFolder();
              else if (e.key === "Escape") creatingIn = null;
            }}
            onblur={commitCreateFolder}
            onclick={(e) => e.stopPropagation()}
            use:selectOnFocus
          />
          <span class="dir-spacer"></span>
        </div>
      {/if}
    {/if}
  {/each}

  <!-- Apple Photos (External Provider) -->
  {#if applePhotos?.supported}
    <ApplePhotosSection
      {applePhotos}
      open={isCatExpanded(APPLE_PHOTOS_ROOT)}
      onToggle={() => toggleCatExpanded(APPLE_PHOTOS_ROOT)}
      {onOpenDir}
      onConnect={onConnectApplePhotos}
      onRefresh={onRefreshApplePhotos}
    />
  {/if}

  <!-- Immich (External Provider) -->
  {#if immich?.connected}
    <ImmichSection
      {immich}
      open={isCatExpanded(IMMICH_ROOT)}
      onToggle={() => toggleCatExpanded(IMMICH_ROOT)}
      {onOpenDir}
      onConnect={onConnectImmich}
      onRefresh={onRefreshImmich}
    />
  {/if}

</div>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    gap: var(--space-d8);
    width: 100%;
    margin-top: calc(var(--space-d4) * 3);
  }
  .lib-row {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    width: 100%;
    margin: 0;
    padding: var(--space-d3) calc(var(--space-d4) * 3);
    cursor: pointer;
    user-select: none;
    font-size: 11px;
    font-weight: 600;
  }
  .lib-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    letter-spacing: -0.01em;
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
    background: var(--color-surface);
  }
  .indexing {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d4);
    font-variant-numeric: tabular-nums;
    font-size: 10px;
  }
  .idx-count {
    min-width: 2.5em;
  }
  .dir-row {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    width: 100%;
    margin: 0;
    padding: var(--space-d4) calc(var(--space-d4) * 3);
    cursor: pointer;
    user-select: none;
    font-size: 11px;
    border-radius: var(--radius-sm);
  }
  .dir-row:hover {
    background: var(--color-surface);
  }
  .dir-row.is-drop-target {
    background: var(--color-accent);
    color: var(--color-on-accent);
  }
  .disc {
    width: 12px;
    height: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-muted);
  }
  .dir-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
  .dir-rename {
    box-sizing: border-box;
    flex: 1;
    min-width: 0;
    padding: 1px 4px;
    font-size: 11px;
  }
  .story-slot {
    width: 8px;
    height: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .virtual-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-d8);
    margin-bottom: var(--space-d4);
  }
  .section-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-d2) calc(var(--space-d4) * 3);
    margin-top: var(--space-d4);
  }
  .section-title {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--color-muted);
    text-transform: uppercase;
  }
  .del-col-btn {
    all: unset;
    cursor: pointer;
    display: none;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: var(--radius-sm);
    color: var(--color-muted);
  }
  .virtual-row:hover .del-col-btn {
    display: flex;
  }
  .del-col-btn:hover {
    color: var(--color-destructive, #ff453a);
    background: var(--color-surface);
  }
  .virtual-row {
    font-weight: 500;
  }
  .virtual-row .disc {
    color: var(--color-accent, var(--color-foreground));
  }
</style>
