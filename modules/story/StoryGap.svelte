<script>
  import { Icon } from "@modules/core";

  /**
   * @typedef {Object} Props
   * @property {number} rowIndex
   * @property {boolean} [isFirst]
   * @property {boolean} [isEnd]
   * @property {boolean} [isOver]
   * @property {(event: DragEvent, index: number) => void} [onDragEnter]
   * @property {(event: DragEvent, index: number) => void} [onDragOver]
   * @property {(index: number) => void} [onDragLeave]
   * @property {(event: DragEvent, index: number) => void} [onDrop]
   * @property {(index: number) => void} [onAddProseAt]
   */

  /** @type {Props} */
  let {
    rowIndex,
    isFirst = false,
    isEnd = false,
    isOver = false,
    onDragEnter = () => {},
    onDragOver = () => {},
    onDragLeave = () => {},
    onDrop = () => {},
    onAddProseAt = () => {},
  } = $props();
</script>

<div
  class="gap"
  class:end={isEnd}
  class:first={isFirst}
  class:over={isOver}
  role="separator"
  data-reveal-host={!isEnd ? true : undefined}
  ondragenter={(e) => onDragEnter(e, rowIndex)}
  ondragover={(e) => onDragOver(e, rowIndex)}
  ondragleave={() => onDragLeave(rowIndex)}
  ondrop={(e) => onDrop(e, rowIndex)}
>
  {#if !isEnd}
    <button class="gap-add small" data-reveal onclick={() => onAddProseAt(rowIndex)} title="Insert a paragraph here">
      <Icon name="plus" size="var(--icon-sm)" /><span>Paragraph</span>
    </button>
  {/if}
</div>

<style>
  .gap {
    position: relative;
    height: 32px;
    margin: var(--space-d4) 0;
    flex-shrink: 0;
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background var(--duration-instant) var(--ease-soft);
  }
  .gap::before {
    content: "";
    position: absolute;
    left: 20%;
    right: 20%;
    height: 1px;
    background: color-mix(in srgb, var(--color-border) 40%, transparent);
    opacity: 0;
    transition: opacity var(--duration-fast) var(--ease-soft);
    pointer-events: none;
  }
  .gap:hover::before {
    opacity: 1;
  }
  .gap.end {
    height: 40px;
  }
  .gap.over {
    background: color-mix(in srgb, var(--color-accent) 25%, transparent);
  }
  .gap-add {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--space-d3);
    padding: var(--space-d4) calc(var(--space-d4) * 3);
    z-index: 2;
  }
  .gap.first .gap-add {
    opacity: 0.6;
    transform: scale(1);
  }

  :global(.composer.dragging) .gap-add {
    opacity: 0;
    pointer-events: none;
  }
</style>
