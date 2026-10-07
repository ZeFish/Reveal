<script>
  import { Icon } from "@modules/core";

  /** @import { Block } from "./storyParser.js" */

  /**
   * @typedef {Object} Props
   * @property {Block} block
   * @property {number} rowIndex
   * @property {number} totalRows
   * @property {boolean} [isDragging]
   * @property {(id: string, text: string) => void} [onTextChange]
   * @property {(from: number, to: number) => void} [onMoveRow]
   * @property {(id: string) => void} [onRemove]
   * @property {(id: string, event: DragEvent) => void} [onDragStart]
   * @property {() => void} [onDragEnd]
   */

  /** @type {Props} */
  let {
    block,
    rowIndex,
    totalRows,
    isDragging = false,
    onTextChange = () => {},
    onMoveRow = () => {},
    onRemove = () => {},
    onDragStart = () => {},
    onDragEnd = () => {},
  } = $props();

  /**
   * Auto-expand textarea to fit its text without scrollbars or resize handles.
   * @param {HTMLTextAreaElement} node
   */
  function autoExpand(node) {
    const update = () => {
      node.style.height = "auto";
      node.style.height = `${node.scrollHeight}px`;
    };
    node.addEventListener("input", update);
    requestAnimationFrame(update);
    return {
      update,
      destroy() {
        node.removeEventListener("input", update);
      },
    };
  }
</script>

<div
  class="prose-row" data-reveal-host
  class:dragging={isDragging}
  data-row-first={block.id}
>
  <div class="prose-container">
    <button
      type="button"
      class="row-grip ghost icon small" data-reveal
      draggable="true"
      ondragstart={(e) => onDragStart(block.id, e)}
      ondragend={onDragEnd}
      title="Drag to move this text"
      aria-label="Move this paragraph"
    >
      <Icon name="dots-six-vertical" size="var(--icon-lg)" />
    </button>

    <textarea
      use:autoExpand
      class="prose"
      rows="1"
      placeholder="Write a paragraph, a thought, or the story of a moment…"
      value={block.text}
      oninput={(e) => onTextChange(block.id, e.currentTarget.value)}
    ></textarea>

    <div class="row-side-actions" data-reveal>
      {#if rowIndex > 0}
        <button
          type="button"
          class="row-action-btn"
          onclick={() => onMoveRow(rowIndex, rowIndex - 1)}
          title="Move this paragraph up"
          aria-label="Move this paragraph up"
        >
          <Icon name="caret-up" size="var(--icon-sm)" />
        </button>
      {/if}
      {#if rowIndex < totalRows - 1}
        <button
          type="button"
          class="row-action-btn"
          onclick={() => onMoveRow(rowIndex, rowIndex + 1)}
          title="Move this paragraph down"
          aria-label="Move this paragraph down"
        >
          <Icon name="caret-down" size="var(--icon-sm)" />
        </button>
      {/if}
      <button
        type="button"
        class="row-action-btn row-remove"
        onclick={() => onRemove(block.id)}
        title="Remove this text"
        aria-label="Delete this paragraph"
      >
        <Icon name="x" size="var(--icon-sm)" />
      </button>
    </div>
  </div>
</div>

<style>
  .prose-row {
    position: relative;
    margin: calc(var(--space-d4) * 3) 0;
    display: flex;
    justify-content: center;
  }
  .prose-row.dragging {
    opacity: 0.35;
  }
  .prose-container {
    position: relative;
    width: 100%;
    max-width: 44rem;
    margin: 0 auto;
    display: flex;
    align-items: flex-start;
  }
  .row-grip {
    position: absolute;
    top: 14px;
    left: -32px;
    box-sizing: border-box;
    cursor: grab;
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    z-index: 3;
  }
  .row-grip:active {
    cursor: grabbing;
  }

  .row-side-actions {
    position: absolute;
    top: 14px;
    right: -34px;
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
    z-index: 3;
  }
  .row-action-btn {
    box-sizing: border-box;
    cursor: pointer;
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .prose {
    display: block;
    box-sizing: border-box;
    width: 100%;
    margin: 0 auto;
    padding: var(--space) calc(var(--space-d4) * 5);
    resize: none;
    overflow: hidden;
    text-align: left;
  }
</style>
