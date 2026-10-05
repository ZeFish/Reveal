<script>
  import ContextMenu from "@stnd/ui/ContextMenu.svelte";
  import MenuItem from "@stnd/ui/ContextMenuItem.svelte";
  import MenuLabel from "@stnd/ui/ContextMenuLabel.svelte";
  import MenuSeparator from "@stnd/ui/ContextMenuSeparator.svelte";
  import AlertDialog from "@stnd/ui/AlertDialog.svelte";
  import { removeLibraryNote } from "@modules/library";

  /**
   * @typedef {Object} Props
   * @property {{ path: string, label: string, x: number, y: number } | null} [folderMenu]
   * @property {{ path: string, name: string } | null} [libraryToRemove]
   * @property {number} [selectedCount]
   * @property {string | null} [root]
   * @property {string[]} [roots]
   * @property {string | null} [importDir]
   * @property {boolean} [scanning]
   * @property {boolean} [menuPathIsLibrary]
   * @property {() => void} [onCloseMenu]
   * @property {() => void} [onCancelRemoveLibrary]
   * @property {(path: string) => void} [onConfirmRemoveLibrary]
   * @property {(path: string) => void} [onMoveSelectedPhotos]
   * @property {(path: string) => void} [onRevealDir]
   * @property {(path: string) => void} [onSetImportDir]
   * @property {(path: string) => void} [onRescanDir]
   * @property {(path: string) => void} [onTidyFolder]
   * @property {(path: string) => void} [onBeginCreateFolder]
   * @property {(path: string, label: string) => void} [onBeginRename]
   * @property {(payload: { path: string, name: string }) => void} [onSelectRemoveLibrary]
   */

  /** @type {Props} */
  let {
    folderMenu = $bindable(null),
    libraryToRemove = $bindable(null),
    selectedCount = 0,
    root = null,
    roots = [],
    importDir = null,
    scanning = false,
    menuPathIsLibrary = false,
    onCloseMenu = () => {},
    onCancelRemoveLibrary = () => {},
    onConfirmRemoveLibrary = () => {},
    onMoveSelectedPhotos = () => {},
    onRevealDir = () => {},
    onSetImportDir = () => {},
    onRescanDir = () => {},
    onTidyFolder = () => {},
    onBeginCreateFolder = () => {},
    onBeginRename = () => {},
    onSelectRemoveLibrary = () => {},
  } = $props();

  /** @param {(path: string) => void} action */
  function runFolderAction(action) {
    const path = folderMenu?.path;
    folderMenu = null;
    onCloseMenu();
    if (path) action(path);
  }
</script>

<AlertDialog
  open={!!libraryToRemove}
  title={libraryToRemove ? `Remove “${libraryToRemove.name}” from Reveal?` : ""}
  description={libraryToRemove ? removeLibraryNote(libraryToRemove.path, roots) : ""}
  confirmLabel="Remove library" cancelLabel="Keep it" intent="danger"
  onconfirm={() => {
    const l = libraryToRemove;
    libraryToRemove = null;
    if (l) onConfirmRemoveLibrary(l.path);
  }}
  oncancel={() => {
    libraryToRemove = null;
    onCancelRemoveLibrary();
  }}
/>

{#if folderMenu}
  <ContextMenu open position={folderMenu} label={`${folderMenu.label} actions`} onclose={() => { folderMenu = null; onCloseMenu(); }}>
  {#snippet content()}
    <MenuLabel label={folderMenu?.label} />
    <MenuSeparator />

    {#if selectedCount > 0}
      <MenuItem
        onclick={() => {
          const path = folderMenu?.path;
          folderMenu = null;
          onCloseMenu();
          if (path) onMoveSelectedPhotos(path);
        }}
      >
        Move {selectedCount} photo{selectedCount > 1 ? 's' : ''} here
      </MenuItem>
      <MenuSeparator />
    {/if}

    <MenuItem label="Show in Finder" onclick={() => runFolderAction(onRevealDir)} />
    <MenuItem
      onclick={() => runFolderAction(onSetImportDir)}
      title="Future imports will use this folder"
    >
      {folderMenu?.path === importDir ? "✓ Active import folder" : "Set as import folder"}
    </MenuItem>
    <MenuItem label="Reindex folder" onclick={() => runFolderAction(onRescanDir)} disabled={scanning} />
    <MenuItem
      label="Tidy folder…"
      onclick={() => runFolderAction(onTidyFolder)}
    />

    <MenuSeparator />

    <MenuItem
        onclick={() => {
          const path = folderMenu?.path;
          folderMenu = null;
          onCloseMenu();
          if (path) onBeginCreateFolder(path);
        }}
    >
      New folder…
    </MenuItem>

    {#if folderMenu?.path !== root && !menuPathIsLibrary}
      <MenuItem
        onclick={() => {
          const path = folderMenu?.path;
          const name = folderMenu?.label;
          folderMenu = null;
          onCloseMenu();
          if (path && name) onBeginRename(path, name);
        }}
      >
        Rename…
      </MenuItem>
    {/if}

    {#if menuPathIsLibrary}
      <MenuSeparator />
      <MenuItem
        onclick={() => {
          const path = folderMenu?.path;
          const name = folderMenu?.label;
          folderMenu = null;
          onCloseMenu();
          if (path) {
            libraryToRemove = { path, name: name || path };
            onSelectRemoveLibrary({ path, name: name || path });
          }
        }}
      >
        Remove library…
      </MenuItem>
    {/if}
  {/snippet}
  </ContextMenu>
{/if}
