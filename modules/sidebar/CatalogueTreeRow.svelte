<script>
  import { Icon } from "@modules/core";
  import CatalogueTreeRow from "./CatalogueTreeRow.svelte";

  /** @typedef {import('./treeOperations.js').TreeNode} TreeNode */

  /**
   * @typedef {Object} Props
   * @property {TreeNode} node
   * @property {number} [depth]
   * @property {(node: TreeNode) => boolean} isCurrent
   * @property {(rel: string) => boolean} isExpanded
   * @property {(abs: string) => boolean} isImportDest
   * @property {(abs: string) => boolean} isImportBranch
   * @property {(node: TreeNode) => boolean} hasChildren
   * @property {(abs: string) => Array<{ abs: string, name: string, parentAbs: string }>} ghostsUnder
   * @property {string | null} [dropTarget]
   * @property {string | null} [draggingFolder]
   * @property {string | null} [renamingPath]
   * @property {string} [renameValue]
   * @property {string | null} [creatingIn]
   * @property {string} [createValue]
   * @property {Set<string>} [storyDirs]
   * @property {(node: TreeNode) => void} onNavigate
   * @property {(rel: string) => void} onToggle
   * @property {(event: MouseEvent, abs: string, name: string) => void} onOpenFolderMenu
   * @property {(event: DragEvent, node: TreeNode) => void} onFolderDragStart
   * @property {() => void} onFolderDragEnd
   * @property {(event: DragEvent, abs: string) => void} onRowDragOver
   * @property {(abs: string) => void} onRowDragLeave
   * @property {(event: DragEvent, abs: string) => void} onRowDrop
   * @property {() => void} onCommitRename
   * @property {() => void} onCancelRename
   * @property {() => void} onCommitCreateFolder
   * @property {() => void} onCancelCreateFolder
   * @property {(abs: string) => void} onOpenDir
   */

  /** @type {Props} */
  let {
    node,
    depth = 0,
    isCurrent,
    isExpanded,
    isImportDest,
    isImportBranch,
    hasChildren,
    ghostsUnder,
    dropTarget = null,
    draggingFolder = null,
    renamingPath = null,
    renameValue = $bindable(""),
    creatingIn = null,
    createValue = $bindable("New Folder"),
    storyDirs = new Set(),
    onNavigate,
    onToggle,
    onOpenFolderMenu,
    onFolderDragStart,
    onFolderDragEnd,
    onRowDragOver,
    onRowDragLeave,
    onRowDrop,
    onCommitRename,
    onCancelRename,
    onCommitCreateFolder,
    onCancelCreateFolder,
    onOpenDir,
  } = $props();

  /** @param {HTMLInputElement} el */
  const selectOnFocus = (el) => {
    el.focus();
    el.select();
  };
</script>

<div
  class="item dir-row"
  class:import-dest={isImportDest(node.abs)}
  class:import-branch={isImportBranch(node.abs)}
  class:is-drop-target={dropTarget === node.abs}
  class:is-dragging={draggingFolder === node.abs}
  style="--depth: {depth}"
  role="button"
  tabindex="0"
  draggable={true}
  aria-label={node.name}
  aria-current={isCurrent(node) ? "true" : undefined}
  onclick={() => onNavigate(node)}
  oncontextmenu={(event) => onOpenFolderMenu(event, node.abs, node.name)}
  ondragstart={(event) => onFolderDragStart(event, node)}
  ondragend={onFolderDragEnd}
  ondragover={(event) => onRowDragOver(event, node.abs)}
  ondragleave={() => onRowDragLeave(node.abs)}
  ondrop={(event) => onRowDrop(event, node.abs)}
  onkeydown={(e) => {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || (e.key === " " && e.currentTarget.matches(":focus-visible"))) {
      e.preventDefault();
      e.stopPropagation();
      onNavigate(node);
    }
  }}
>
  {#if hasChildren(node)}
    <button
      class="disc"
      class:open={isExpanded(node.rel)}
      onclick={(e) => {
        e.stopPropagation();
        onToggle(node.rel);
      }}
      aria-label={`${isExpanded(node.rel) ? "Collapse" : "Expand"} ${node.name}`}
      aria-expanded={isExpanded(node.rel)}
    >
      <Icon name="caret-right" size="9px" />
    </button>
  {:else}
    <span class="disc"></span>
  {/if}
  {#if renamingPath === node.abs}
    <input
      class="dir-rename"
      value={renameValue}
      oninput={(e) => (renameValue = e.currentTarget.value)}
      onkeydown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") onCommitRename();
        else if (e.key === "Escape") onCancelRename();
      }}
      onblur={onCommitRename}
      onclick={(e) => e.stopPropagation()}
      use:selectOnFocus
    />
  {:else}
    <span class="dir-name" title={node.abs}>{node.name}</span>
  {/if}
  <span class="dir-spacer"></span>
  <span class="dir-count">{node.count > 0 ? node.count : ""}</span>
  <span class="story-slot">
    {#if storyDirs.has(node.abs)}
      <span class="story-dot" title="This folder contains a story"></span>
    {/if}
  </span>
</div>

{#if hasChildren(node) && isExpanded(node.rel)}
  {#each node.children as child (child.rel)}
    <CatalogueTreeRow
      node={child}
      depth={depth + 1}
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
      {onNavigate}
      {onToggle}
      {onOpenFolderMenu}
      {onFolderDragStart}
      {onFolderDragEnd}
      {onRowDragOver}
      {onRowDragLeave}
      {onRowDrop}
      {onCommitRename}
      {onCancelRename}
      {onCommitCreateFolder}
      {onCancelCreateFolder}
      {onOpenDir}
    />
  {/each}
  {#each ghostsUnder(node.abs) as g (g.abs)}
    <div
      class="item dir-row"
      class:is-drop-target={dropTarget === g.abs}
      style="--depth: {depth + 1}"
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
{/if}

{#if creatingIn === node.abs}
  <div class="dir-row" style="--depth: {depth + 1}">
    <span class="disc"></span>
    <input
      class="dir-rename"
      value={createValue}
      oninput={(e) => (createValue = e.currentTarget.value)}
      onkeydown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") onCommitCreateFolder();
        else if (e.key === "Escape") onCancelCreateFolder();
      }}
      onblur={onCommitCreateFolder}
      onclick={(e) => e.stopPropagation()}
      use:selectOnFocus
    />
    <span class="dir-spacer"></span>
  </div>
{/if}

<style>
  .dir-row {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    width: 100%;
    margin: 0;
    padding: var(--space-d4) calc(var(--space-d4) * 3);
    padding-left: calc(var(--space-d4) * 3 + var(--depth, 0) * var(--space));
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
  .dir-row.is-dragging {
    opacity: 0.4;
  }
  .disc {
    width: 12px;
    height: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    color: var(--color-muted);
    transition: transform var(--duration-fast);
  }
  .disc.open {
    transform: rotate(90deg);
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
  .dir-spacer {
    flex: 1;
  }
  .dir-count {
    font-variant-numeric: tabular-nums;
    color: var(--color-muted);
    font-size: 10px;
    margin-right: var(--space-d4);
  }
  .story-slot {
    width: 8px;
    height: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .story-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-accent);
  }
</style>
