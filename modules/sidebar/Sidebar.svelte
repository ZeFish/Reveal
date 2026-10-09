<script>
  // The floating sidebar — a 1:1 port of the Swift `sidebarCard` +
  // `folderBrowser` + `FolderTree` (CullView.swift / FolderTree.swift):
  // full-height elevated card, the brand cluster (traffic lights + toggles +
  // wordmark) riding its top, FRAMES/EDITORIAL tabs, "ALL LIBRARY",
  // one section per catalogue, the tree with status dots and story dots,
  // then LIBRARY and the Garden account row at the bottom.
  import BrandCluster from "./BrandCluster.svelte";
  import SidebarTabs from "./SidebarTabs.svelte";
  import CatalogueTree from "./CatalogueTree.svelte";
  import LibraryNoteDrawer from "./LibraryNoteDrawer.svelte";
  import EditorialSidebar from "./EditorialSidebar.svelte";
  import GardenAccountRow from "./GardenAccountRow.svelte";
  import FolderModals from "./FolderModals.svelte";

  /**
   * @typedef {Object} Props
   * @property {string | null} [root]
   * @property {string[]} [roots]
   * @property {any[]} dirs
   * @property {string | null} curDir
   * @property {Array<import('@modules/core').Collection>} [collections]
   * @property {boolean} [scanning]
   * @property {{dirs: number, frames: number} | null} [indexProgress]
   * @property {boolean} [previewFilter]
   * @property {Set<string>} [storyDirs]
   * @property {() => void} [onDevelopStory]
   * @property {() => void} [onPublishStory]
   * @property {() => void} [onExportLocalStory]
   * @property {boolean} [publishing]
   * @property {boolean} [storyPublished]
   * @property {string | null} [publishStatus]
   * @property {string | null} [gardenUrl]
   * @property {boolean} [focusOn]
   * @property {string} [catalogContent]
   * @property {string | null} [importDir]
   * @property {boolean} [floating]
   * @property {(path: string) => void} onOpenDir
   * @property {() => void} onOpenLibrary
   * @property {() => void} [onRescan]
   * @property {(path: string) => void} [onRescanDir]
   * @property {(path: string) => void} [onTidyFolder]
   * @property {(path: string) => Promise<void> | void} [onRemoveLibrary]
   * @property {(path: string) => void} [onRevealDir]
   * @property {() => void} onAddLocation
   * @property {(path: string) => void} [onSetImportDir]
   * @property {() => void} [onOpenNote]
   * @property {() => void} [onTogglePreview]
   * @property {() => void} [onToggleSidebar]
   * @property {() => void} [onToggleFocus]
   * @property {() => void} [onToggleAppearance]
   * @property {() => void} [onShowShortcuts]
   * @property {() => void} [onShowSettings]
   * @property {(value: string) => void} [onCatalogChange]
   * @property {any} [garden]
   * @property {(key: string) => Promise<any> | void} [onGardenSignIn]
   * @property {() => Promise<any> | void} [onGardenSignOut]
   * @property {(url: string) => void} [onOpenUrl]
   * @property {(paths: string[], destAbs: string) => void} [onMovePhotos]
   * @property {number} [selectedCount]
   * @property {(path: string) => void} [onMoveSelectedPhotos]
   * @property {(path: string, newName: string) => Promise<void> | void} [onRenameDir]
   * @property {(parentAbs: string, name: string) => Promise<string | null | void> | string | null | void} [onCreateFolder]
   * @property {(srcAbs: string, destAbs: string) => Promise<void> | void} [onMoveDir]
   * @property {import('@modules/apple-photos/applePhotosTree.js').PhotoLibrary | null} [applePhotos]
   * @property {() => void} [onConnectApplePhotos]
   * @property {() => void} [onRefreshApplePhotos]
   * @property {any} [immich]
   * @property {() => void} [onConnectImmich]
   * @property {() => void} [onRefreshImmich]
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
    previewFilter = false,
    storyDirs = new Set(),
    onDevelopStory = () => {},
    onPublishStory = () => {},
    onExportLocalStory = () => {},
    publishing = false,
    storyPublished = false,
    publishStatus = null,
    gardenUrl = null,
    focusOn = false,
    catalogContent = "",
    importDir = null,
    floating = false,
    onOpenDir = () => {},
    onOpenLibrary = () => {},
    onRescan = () => {},
    onRescanDir = () => {},
    onTidyFolder = () => {},
    onRemoveLibrary = () => {},
    onRevealDir = () => {},
    onAddLocation = () => {},
    onSetImportDir = () => {},
    onOpenNote = () => {},
    onTogglePreview = () => {},
    onToggleSidebar = () => {},
    onToggleFocus = () => {},
    onToggleAppearance = () => {},
    onShowShortcuts = () => {},
    onShowSettings = () => {},
    onCatalogChange = () => {},
    garden = null,
    onGardenSignIn = () => {},
    onGardenSignOut = () => {},
    onOpenUrl = () => {},
    onMovePhotos = () => {},
    selectedCount = 0,
    onMoveSelectedPhotos = () => {},
    onRenameDir = () => {},
    onCreateFolder = () => {},
    onMoveDir = () => {},
    applePhotos = null,
    onConnectApplePhotos = () => {},
    onRefreshApplePhotos = () => {},
    immich = null,
    onConnectImmich = () => {},
    onRefreshImmich = () => {},
  } = $props();

  /** @type {{ path: string, label: string, x: number, y: number } | null} */
  let folderMenu = $state(null);
  /** @type {{path: string, name: string} | null} */
  let libraryToRemove = $state(null);

  /** @type {string | null} */
  let renamingPath = $state(null);
  let renameValue = $state("");

  /** @type {string | null} */
  let creatingIn = $state(null);
  let createValue = $state("New Folder");

  const isLibrary = $derived(!!root && curDir === root && !applePhotos?.active);

  const menuPathIsLibrary = $derived.by(() => {
    const path = folderMenu?.path;
    if (!path) return false;
    const known = roots?.length ? roots : root ? [root] : [];
    return known.includes(path);
  });

  /**
   * @param {MouseEvent} event
   * @param {string} path
   * @param {string} label
   */
  function openFolderMenu(event, path, label) {
    event.preventDefault();
    event.stopPropagation();
    folderMenu = {
      path,
      label,
      x: event.clientX,
      y: event.clientY,
    };
  }

  /** @param {string} path */
  function beginCreateFolder(path) {
    creatingIn = path;
    createValue = "New Folder";
  }

  /**
   * @param {string} path
   * @param {string} name
   */
  function beginRename(path, name) {
    renamingPath = path;
    renameValue = name;
  }
</script>

<nav class="pane" class:floating>
  {#if !floating}
    <BrandCluster
      {focusOn}
      {onToggleSidebar}
      {onToggleFocus}
      {onToggleAppearance}
      {onShowShortcuts}
    />
  {/if}

  <SidebarTabs
    {previewFilter}
    {isLibrary}
    applePhotosActive={!!applePhotos?.active}
    {onTogglePreview}
  />

  {#if !previewFilter}
    <CatalogueTree
      {root}
      {roots}
      {dirs}
      {curDir}
      {collections}
      {scanning}
      {indexProgress}
      {isLibrary}
      {storyDirs}
      {importDir}
      {applePhotos}
      {immich}
      bind:renamingPath
      bind:renameValue
      bind:creatingIn
      bind:createValue
      {onOpenDir}
      {onOpenLibrary}
      {onAddLocation}
      {onConnectApplePhotos}
      {onRefreshApplePhotos}
      {onConnectImmich}
      {onRefreshImmich}
      {onRescanDir}
      {onMovePhotos}
      {onMoveDir}
      {onRenameDir}
      {onCreateFolder}
      onOpenFolderMenu={openFolderMenu}
    />

    <LibraryNoteDrawer
      {catalogContent}
      {onCatalogChange}
      {onOpenNote}
    />
  {:else}
    <EditorialSidebar
      {curDir}
      signedIn={!!garden?.signed_in}
      {publishing}
      {storyPublished}
      {publishStatus}
      {gardenUrl}
      {onPublishStory}
      {onDevelopStory}
      {onExportLocalStory}
      {onOpenUrl}
    />
  {/if}

  <div class="divider"></div>

  <GardenAccountRow
    {garden}
    {onGardenSignIn}
    {onGardenSignOut}
    {onOpenUrl}
    {onShowSettings}
  />
</nav>

<FolderModals
  bind:folderMenu
  bind:libraryToRemove
  {selectedCount}
  {root}
  {roots}
  {importDir}
  {scanning}
  {menuPathIsLibrary}
  onMoveSelectedPhotos={(path) => onMoveSelectedPhotos(path)}
  onRevealDir={(path) => onRevealDir(path)}
  onSetImportDir={(path) => onSetImportDir(path)}
  onRescanDir={(path) => onRescanDir(path)}
  onTidyFolder={(path) => onTidyFolder(path)}
  onConfirmRemoveLibrary={(path) => onRemoveLibrary(path)}
  onBeginCreateFolder={(path) => beginCreateFolder(path)}
  onBeginRename={(path, name) => beginRename(path, name)}
/>

<style>
  nav {
    width: 224px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;

    /* The sidebar's one grid. Every row — All Library, the collections, a catalogue's header, a
       folder at any depth, Library, the account — is laid on the same columns, so a name, a
       count and a button sit at the same x from the top of the panel to the bottom:
         [ gutter | lead | name ................ | count | gutter ]
       `lead` carries the disclosure arrow or the icon. The count goes to the far right, so counts
       line up on their own. What is rare takes no column: the story dot sits in the left gutter
       (one rail of dots), and the + / refresh button replaces the count while the row is hovered. */
    --sb-pad: var(--space-d2);
    --sb-lead: 14px;
    --sb-indent: 10px;
  }
  /* A focus ring drawn outside a row is clipped by the scrolling tree on the sides and overlaps
     the rows above and below; inside it stays where the row is. */
  nav :global(:focus-visible) {
    outline-offset: -2px;
  }
  nav.floating {
    height: min(480px, calc(100vh - 60px));
    margin: 0;
  }

  .divider {
    height: 1px;
    background: var(--color-border);
    flex-shrink: 0;
  }
</style>
