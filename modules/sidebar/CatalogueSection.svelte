<script>
  import { Icon } from "@modules/core";

  /**
   * @typedef {Object} Props
   * @property {string} cat
   * @property {string} name
   * @property {number} total
   * @property {boolean} open
   * @property {import('@modules/core').Source} [source]
   * @property {boolean} [isDest]
   * @property {boolean} [isBranch]
   * @property {boolean} [isCurrent]
   * @property {boolean} [scanning]
   * @property {() => void} [onToggle]
   * @property {() => void} [onOpen]
   * @property {() => void} [onRefresh]
   * @property {(event: MouseEvent) => void} [onContextMenu]
   */

  /** @type {Props} */
  let {
    cat,
    name,
    total,
    source,
    open,
    isDest = false,
    isBranch = false,
    isCurrent = false,
    scanning = false,
    onToggle = () => {},
    onOpen = () => {},
    onRefresh = () => {},
    onContextMenu = () => {},
  } = $props();
</script>

<div class="section">
  <button
    class="cat-disc"
    onclick={(e) => {
      e.stopPropagation();
      onToggle();
    }}
    aria-label={`${open ? "Collapse" : "Expand"} ${name}`}
    aria-expanded={open}
    title={open ? "Collapse catalogue" : "Expand catalogue"}
  >
    <span class="disc" class:open><Icon name="caret-right" size="9px" /></span>
  </button>
  <button
    class="section-main ghost"
    class:import-dest={isDest}
    class:import-branch={isBranch}
    title={cat}
    aria-current={isCurrent ? "true" : undefined}
    onclick={() => onOpen()}
    oncontextmenu={(event) => onContextMenu(event)}
  >
    {#if source?.icon}
      <Icon name={source.icon} size="10px" class="source-icon" />
    {/if}
    <span class="section-name">{name}</span>
  </button>
  <span class="dir-spacer"></span>
  <span class="dir-count">{total.toLocaleString("en-CA")}</span>
  <button
    class="add-btn"
    onclick={(e) => {
      e.stopPropagation();
      onRefresh();
    }}
    disabled={scanning}
    title={`Reindex ${name}`}
    aria-label={`Reindex ${name}`}
    aria-busy={scanning}
  >
    <Icon name="arrows-clockwise" size="9px" class={scanning ? "spin" : ""} />
  </button>
</div>

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
  :global(.source-icon) {
    margin-right: var(--space-d4);
    opacity: 0.7;
    vertical-align: middle;
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
</style>
