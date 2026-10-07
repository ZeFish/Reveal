<script>
  import { Icon } from "@modules/core";

  /** @import { Block, DropPlace } from "./storyParser.js" */
  /** @typedef {{ name: string, path: string, previewVersion?: number, capture_at?: number | string }} Frame */
  /** @typedef {ReturnType<import("./storyParser.js").storyRows>[number]} StoryRow */

  /**
   * @typedef {Object} Props
   * @property {StoryRow & { type: "photos" }} row
   * @property {string | null} [dragId]
   * @property {{ id: string, place: DropPlace } | null} [dropTarget]
   * @property {Map<string, Frame>} [frameByStem]
   * @property {(path: string, version: number) => string} thumbUrl
   * @property {(blocks: Block[]) => Array<{ block: Block, w: number, h: number }>} rowLayout
   * @property {(stem: string, event: Event) => void} [onImgLoad]
   * @property {(id: string, event: DragEvent) => void} [onDragStart]
   * @property {() => void} [onDragEnd]
   * @property {(event: DragEvent, id: string) => void} [onCellDragOver]
   * @property {(event: DragEvent, id: string) => void} [onCellDrop]
   * @property {(id: string) => void} [onBreakOut]
   * @property {(id: string) => void} [onRemove]
   * @property {(id: string, text: string) => void} [onSetText]
   * @property {(id: string) => void} [onClearDropTarget]
   */

  /** @type {Props} */
  let {
    row,
    dragId = null,
    dropTarget = null,
    frameByStem = new Map(),
    thumbUrl,
    rowLayout,
    onImgLoad = () => {},
    onDragStart = () => {},
    onDragEnd = () => {},
    onCellDragOver = () => {},
    onCellDrop = () => {},
    onBreakOut = () => {},
    onRemove = () => {},
    onSetText = () => {},
    onClearDropTarget = () => {},
  } = $props();
</script>

<div class="photo-row" style="gap: 8px;" data-row-first={row.blocks[0].id}>
  {#each rowLayout(row.blocks) as cell (cell.block.id)}
    {@const f = frameByStem.get(cell.block.stem)}
    <div
      class="photo-cell"
      class:dragging={dragId === cell.block.id}
      class:drop-left={dropTarget?.id === cell.block.id && dropTarget?.place === "left"}
      class:drop-right={dropTarget?.id === cell.block.id && dropTarget?.place === "right"}
      class:drop-above={dropTarget?.id === cell.block.id && dropTarget?.place === "above"}
      class:drop-below={dropTarget?.id === cell.block.id && dropTarget?.place === "below"}
      style="width: {cell.w}px;"
      draggable="true"
      role="listitem"
      ondragstart={(e) => onDragStart(cell.block.id, e)}
      ondragend={onDragEnd}
      ondragenter={(e) => onCellDragOver(e, cell.block.id)}
      ondragover={(e) => onCellDragOver(e, cell.block.id)}
      ondragleave={() => dropTarget?.id === cell.block.id && onClearDropTarget(cell.block.id)}
      ondrop={(e) => onCellDrop(e, cell.block.id)}
    >
      <div class="frame" data-reveal-host style="height: {cell.h}px;">
        {#if f}
          <img
            src={thumbUrl(f.path, f.previewVersion ?? 0)}
            alt={cell.block.stem}
            draggable="false"
            onload={(e) => onImgLoad(cell.block.stem, e)}
          />
        {:else}
          <div class="missing" title="Photo missing from the folder">{cell.block.stem}</div>
        {/if}
        {#if row.blocks.length > 1}
          <button class="cell-break icon small" data-reveal onclick={() => onBreakOut(cell.block.id)} title="Split onto its own row">
            <Icon name="rows" size="var(--icon-md)" />
          </button>
        {/if}
        <button class="cell-remove icon small" data-reveal onclick={() => onRemove(cell.block.id)} title="Remove from story">
          <Icon name="x" size="var(--icon-sm)" />
        </button>
      </div>
      {#if row.blocks.length === 1}
        <input
          class="caption"
          placeholder="Add a caption…"
          value={cell.block.text}
          oninput={(e) => onSetText(cell.block.id, e.currentTarget.value)}
        />
      {/if}
    </div>
  {/each}
</div>

{#if row.blocks.length > 1}
  {@const last = row.blocks[row.blocks.length - 1]}
  <input
    class="caption"
    placeholder="Add a caption for this row…"
    value={last.text}
    oninput={(e) => onSetText(last.id, e.currentTarget.value)}
  />
{/if}

<style>
  .photo-row {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    margin: var(--space-d2) 0;
  }
  .photo-cell {
    position: relative;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
    cursor: grab;
  }
  .photo-cell.dragging {
    opacity: 0.4;
  }
  .frame {
    position: relative;
    width: 100%;
    border-radius: var(--radius);
    overflow: hidden;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.45), 0 0 0 1px rgba(255, 255, 255, 0.08);
    transition: box-shadow var(--duration-fast) var(--ease-standard), transform var(--duration-fast) var(--ease-standard);
  }
  .photo-cell.drop-left::before,
  .photo-cell.drop-right::after,
  .photo-cell.drop-above::before,
  .photo-cell.drop-below::after {
    content: "";
    position: absolute;
    background: var(--color-accent);
    border-radius: var(--radius);
    z-index: 10;
    pointer-events: none;
  }
  .photo-cell.drop-left::before,
  .photo-cell.drop-right::after {
    top: 0;
    bottom: 0;
    width: 4px;
  }
  .photo-cell.drop-left::before {
    left: -6px;
  }
  .photo-cell.drop-right::after {
    right: -6px;
  }
  .photo-cell.drop-above::before,
  .photo-cell.drop-below::after {
    left: -4px;
    right: -4px;
    height: 4px;
  }
  .photo-cell.drop-above::before {
    top: -6px;
  }
  .photo-cell.drop-below::after {
    bottom: -6px;
  }
  .frame img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    border-radius: inherit;
    pointer-events: none;
    -webkit-user-drag: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .missing {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-surface);
    text-align: center;
    padding: var(--space-d4);
    box-sizing: border-box;
  }
  .cell-remove,
  .cell-break {
    position: absolute;
    top: 8px;
    box-sizing: border-box;
    cursor: pointer;
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    z-index: 2;
  }
  .cell-remove {
    right: 8px;
  }
  .cell-break {
    left: 8px;
  }
  .caption {
    display: block;
    box-sizing: border-box;
    width: 100%;
    text-align: center;
    padding: var(--space-d3) calc(var(--space-d4) * 3);
  }
</style>
