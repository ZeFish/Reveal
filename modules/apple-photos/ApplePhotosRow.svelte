<script>
  import { Icon } from "@modules/core";
  import ApplePhotosRow from "./ApplePhotosRow.svelte";

  /**
   * @typedef {import('./applePhotosTree.js').PhotoCollection} PhotoCollection
   */

  /**
   * @typedef {Object} Props
   * @property {PhotoCollection} collection
   * @property {number} [depth]
   * @property {(id: string) => boolean} isCurrent
   * @property {(id: string) => boolean} isExpanded
   * @property {(id: string) => void} onToggle
   * @property {(id: string) => void} onNavigate
   * @property {boolean} [busy]
   */

  /** @type {Props} */
  let {
    collection,
    depth = 0,
    isCurrent,
    isExpanded,
    onToggle,
    onNavigate,
    busy = false,
  } = $props();

  const hasChildren = $derived(Boolean(collection.children && collection.children.length > 0));
  const expanded = $derived(isExpanded(collection.id));
  const active = $derived(isCurrent(collection.id));
</script>

<div
  class="item dir-row"
  style="--depth: {depth}"
  role="button"
  tabindex="0"
  aria-disabled={busy}
  aria-label={collection.title}
  aria-current={active ? "true" : undefined}
  onclick={() => onNavigate(collection.id)}
  oncontextmenu={(e) => e.preventDefault()}
  onkeydown={(e) => {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || (e.key === " " && e.currentTarget.matches(":focus-visible"))) {
      e.preventDefault();
      e.stopPropagation();
      onNavigate(collection.id);
    }
  }}
>
  {#if hasChildren}
    <button
      class="disc"
      class:open={expanded}
      onclick={(e) => {
        e.stopPropagation();
        onToggle(collection.id);
      }}
      aria-label={`${expanded ? "Collapse" : "Expand"} ${collection.title}`}
      aria-expanded={expanded}
    >
      <Icon name="caret-right" size="var(--icon-sm)" />
    </button>
  {:else}
    <span class="disc"></span>
  {/if}
  <span class="dir-name" title={collection.title}>{collection.title}</span>
  <span class="dir-spacer"></span>
  <span class="dir-count">{collection.count != null && collection.count > 0 ? collection.count.toLocaleString("en-CA") : ""}</span>
</div>

{#if hasChildren && expanded}
  {#each collection.children as child (child.id)}
    <ApplePhotosRow
      collection={child}
      depth={depth + 1}
      {isCurrent}
      {isExpanded}
      {onToggle}
      {onNavigate}
      {busy}
    />
  {/each}
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
    background: var(--color-hover);
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
    /* The name takes the room it needs and gives way (ellipsis) only when the row is full. As
       `flex: 1` it split the free space 50/50 with the spacer, so a name was cut at half the row
       with empty space beside it. */
    flex: 0 1 auto;
    min-width: 0;
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
</style>
