<script>
  import Menu from "@stnd/ui/ContextMenu.svelte";
  import Item from "@stnd/ui/ContextMenuItem.svelte";
  import Checkbox from "@stnd/ui/ContextMenuItemCheckbox.svelte";
  import Label from "@stnd/ui/ContextMenuLabel.svelte";
  import Separator from "@stnd/ui/ContextMenuSeparator.svelte";
  /**
   * @typedef {Object} Props
   * @property {*} [photoMenu]
   * @property {Set<string>} [selectedPaths]
   * @property {any[]} [installedEditors]
   * @property {*} [copiedRecipe]
   * @property {Set<string>} [storySet]
   * @property {(s: string) => string} [stem]
   * @property {() => void} [onClose]
   * @property {(p: string) => void} [onOpenPhoto]
   * @property {(p: string) => void} [onOpenPreview]
   * @property {(p: string) => void} [onRevealInFinder]
   * @property {(p: string, app: string) => void} [onOpenInEditor]
   * @property {(p: string) => void} [onCopyImage]
   * @property {() => void} [onCopySettings]
   * @property {() => void} [onPasteSettings]
   * @property {(n: number) => void} [onRate]
   * @property {(p: string) => void} [onToggleStory]
   * @property {() => void} [onExportSelection]
   * @property {string | null} [gardenUrl]
   * @property {() => void} [onCull]
   * @property {() => void} [onPublishStory]
   * @property {() => void} [onOpenGardenUrl]
   * @property {(p: string) => void} [onDevelopToVault]
   */

  /** @type {Props} */
  let {
    photoMenu = null,
    selectedPaths = new Set(),
    installedEditors = [],
    copiedRecipe = null,
    storySet = new Set(),
    gardenUrl = null,
    stem = (/** @type {string} */ s) => s,
    onClose = () => {},
    onOpenPhoto = (/** @type {string} */ p) => {},
    onOpenPreview = (/** @type {string} */ p) => {},
    onRevealInFinder = (/** @type {string} */ p) => {},
    onOpenInEditor = (/** @type {string} */ p, /** @type {string} */ app) => {},
    onCopyImage = (/** @type {string} */ p) => {},
    onCopySettings = () => {},
    onPasteSettings = () => {},
    onRate = (/** @type {number} */ n) => {},
    onToggleStory = (/** @type {string} */ p) => {},
    onExportSelection = () => {},
    onDevelopToVault = undefined,
    onCull = undefined,
    onPublishStory = undefined,
    onOpenGardenUrl = undefined,
  } = $props();
</script>

{#if photoMenu}
  <Menu open position={photoMenu} label="Photo actions" onclose={onClose}>
  {#snippet content()}
    <Label label={selectedPaths.size > 1 ? `${selectedPaths.size} selected photos` : photoMenu.frame.name} />
    <Separator />
    <Item label="Develop" shortcut="d" onclick={() => onOpenPhoto(photoMenu.frame.path)} />
    <Item label="Preview (Quick Look)" shortcut="Space" onclick={() => onOpenPreview(photoMenu.frame.path)} />
    <Item label="Show in Finder" disabled={photoMenu.frame.path.startsWith("apple-photos://")} onclick={() => onRevealInFinder(photoMenu.frame.path)} />
    {#if installedEditors.length && !photoMenu.frame.path.startsWith("apple-photos://")}
      <Separator />
      <Label label="Open with" />
      {#each installedEditors as [name, appPath]}
        <Item label={name} onclick={() => onOpenInEditor(photoMenu.frame.path, appPath)} />
      {/each}
    {/if}
    <Separator />
    <Item label="Copy image" shortcut="⌘C" onclick={() => onCopyImage(photoMenu.frame.path)} />
    <Item label="Copy settings" shortcut="c" onclick={onCopySettings} />
    <Item label="Paste settings" shortcut="v" disabled={!copiedRecipe} onclick={onPasteSettings} />
    <Separator />
    <Label label="Rating" />
    {#each [0, 1, 2, 3, 4, 5] as rating}
      <Checkbox label={rating === 0 ? "Unrated" : "★".repeat(rating)} checked={(photoMenu.frame.rating || 0) === rating} onCheckedChange={() => onRate(rating)} />
    {/each}
    <Separator />
    <Item label={storySet.has(stem(photoMenu.frame.name)) ? "Remove from collection" : "Add to collection"} shortcut="q" disabled={photoMenu.frame.path.startsWith("apple-photos://")} onclick={() => onToggleStory(photoMenu.frame.path)} />
    <Separator />
    <Item label="Export selection…" shortcut="r" onclick={onExportSelection} />
    {#if onDevelopToVault}
      <Item label={selectedPaths.size > 1 ? "Add selection to daily note" : "Add to daily note (Obsidian)"} onclick={() => onDevelopToVault(photoMenu.frame.path)} />
    {/if}
    {#if onCull || (storySet.size > 0 && onPublishStory) || (gardenUrl && onOpenGardenUrl)}
      <Separator />
    {/if}
    {#if onCull}
      <Item label="AI culling" onclick={onCull} />
    {/if}
    {#if storySet.size > 0 && onPublishStory}
      <Item label={`Publish collection (${storySet.size})`} onclick={onPublishStory} />
    {/if}
    {#if gardenUrl && onOpenGardenUrl}
      <Item label="Open on the web" onclick={onOpenGardenUrl} />
    {/if}
  {/snippet}
  </Menu>
{/if}
