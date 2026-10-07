<script>
  import { Icon } from "@modules/core";
  import { IMMICH_ROOT } from "./immichTree.js";

  /**
   * @typedef {Object} Props
   * @property {{
   *   connected: boolean,
   *   active: boolean,
   *   busy: boolean,
   *   loaded: boolean,
   *   album: string,
   *   albums: Array<{id: string, title: string, count: number}>,
   *   total: number | null
   * }} immich
   * @property {boolean} [open]
   * @property {() => void} [onToggle]
   * @property {(path: string) => void} onOpenDir
   * @property {() => void} [onConnect]
   * @property {() => void} [onRefresh]
   */

  /** @type {Props} */
  let {
    immich,
    open = false,
    onToggle = () => {},
    onOpenDir,
    onConnect = () => {},
    onRefresh = () => {},
  } = $props();

  const isCurrentAll = $derived(!!immich?.active && !immich.album);
  const totalLabel = $derived(immich?.loaded ? (immich.total ?? 0).toLocaleString("en-CA") : "");

  function handleToggle() {
    onToggle();
    if (!open && !immich?.loaded) {
      onConnect();
    }
  }

  function handleOpenAll() {
    if (!open) {
      onToggle();
      if (!immich?.loaded) onConnect();
    }
    onOpenDir(IMMICH_ROOT);
  }
</script>

<div class="section">
  <button
    class="cat-disc"
    onclick={(e) => {
      e.stopPropagation();
      handleToggle();
    }}
    disabled={immich?.busy && !immich?.loaded}
    aria-label={`${open ? "Collapse" : "Expand"} Immich`}
    aria-expanded={open}
    title={open ? "Collapse catalogue" : "Expand catalogue"}
  >
    <span class="disc" class:open><Icon name="caret-right" size="var(--icon-sm)" /></span>
  </button>
  <button
    class="section-main ghost"
    title="Immich"
    aria-current={isCurrentAll ? "true" : undefined}
    disabled={immich?.busy}
    onclick={handleOpenAll}
    oncontextmenu={(e) => e.preventDefault()}
  >
    <span class="section-name">Immich</span>
  </button>
  <span class="dir-spacer"></span>
  <span class="dir-count">{totalLabel}</span>
  <button
    class="add-btn"
    onclick={(e) => {
      e.stopPropagation();
      onRefresh();
    }}
    disabled={immich?.busy}
    title="Refresh Immich"
    aria-label="Refresh Immich"
    aria-busy={immich?.busy}
  >
    <Icon name="arrows-clockwise" size="var(--icon-sm)" class={immich?.busy ? "spin" : ""} />
  </button>
</div>

{#if open && immich?.albums}
  {#each immich.albums as album (album.id)}
    {@const albumPath = IMMICH_ROOT + album.id}
    {@const isCur = !!immich.active && immich.album === album.id}
    <div
      class="item dir-row"
      role="button"
      tabindex="0"
      aria-label={album.title}
      aria-current={isCur ? "true" : undefined}
      onclick={() => onOpenDir(albumPath)}
      oncontextmenu={(e) => e.preventDefault()}
      onkeydown={(e) => {
        if (e.target !== e.currentTarget) return;
        if (e.key === "Enter" || (e.key === " " && e.currentTarget.matches(":focus-visible"))) {
          e.preventDefault();
          e.stopPropagation();
          onOpenDir(albumPath);
        }
      }}
    >
      <span class="disc"></span>
      <span class="dir-name" title={album.title}>{album.title}</span>
      <span class="dir-spacer"></span>
      <span class="dir-count">{(album.count ?? 0).toLocaleString("en-CA")}</span>
    </div>
  {/each}
{/if}

<style>
  .section {
    display: flex;
    align-items: center;
    padding: var(--space-d4) var(--space-d2);
    margin-top: var(--space-d3);
    border-top: var(--border);
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
    background: var(--color-surface);
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
    background: var(--color-surface);
  }
  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
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
</style>
