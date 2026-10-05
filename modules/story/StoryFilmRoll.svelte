<script>
  import { Icon } from "@modules/core";
  import { stemOf } from "./storyParser.js";

  /**
   * @typedef {{ name: string, path: string, previewVersion?: number, capture_at?: number | string }} Frame
   */

  /**
   * @typedef {Object} Props
   * @property {Frame[]} [frames]
   * @property {Set<string>} [storySet]
   * @property {(path: string, version: number) => string} thumbUrl
   * @property {(name: string) => void} [onAddPhoto]
   * @property {(stem: string, event: DragEvent) => void} [onDragStart]
   * @property {() => void} [onDragEnd]
   */

  /** @type {Props} */
  let {
    frames = [],
    storySet = new Set(),
    thumbUrl,
    onAddPhoto = () => {},
    onDragStart = () => {},
    onDragEnd = () => {},
  } = $props();
</script>

{#if frames.length}
  <div class="roll pane">
    <div class="roll-header">
      <Icon name="stack-simple" size="13px" />
      <span class="roll-title">FILM ROLL · {frames.length}</span>
    </div>
    <div class="roll-strip">
      {#each frames as f (f.path)}
        {@const s = stemOf(f.name)}
        <div
          class="roll-cell"
          class:in-story={storySet.has(s)}
          draggable="true"
          role="button"
          tabindex="0"
          ondragstart={(e) => onDragStart(s, e)}
          ondragend={onDragEnd}
          onclick={() => onAddPhoto(f.name)}
          onkeydown={(e) => e.key === "Enter" && onAddPhoto(f.name)}
          title={`Add ${f.name}`}
        >
          <img src={thumbUrl(f.path, f.previewVersion ?? 0)} alt={f.name} loading="lazy" draggable="false" />
          {#if storySet.has(s)}
            <span class="roll-check"><Icon name="check" size="9px" /></span>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .roll {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space);
    margin: 0 var(--window-inset) var(--window-inset);
    padding: calc(var(--space-d4) * 3) calc(var(--space-d4) * 5);
    z-index: 10;
  }
  .roll-header {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    flex-shrink: 0;
    color: var(--color-muted);
  }
  .roll-title {
    user-select: none;
    white-space: nowrap;
  }
  .roll-strip {
    display: flex;
    gap: var(--space-d2);
    overflow-x: auto;
    padding: var(--space-d3);
    scrollbar-width: thin;
  }
  .roll-cell {
    all: unset;
    position: relative;
    flex-shrink: 0;
    width: 60px;
    height: 60px;
    border: none;
    border-radius: var(--radius);
    overflow: visible;
    cursor: pointer;
    box-shadow: var(--shadow);
    transition: all var(--transition-fast);
  }
  .roll-cell:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow-hover);
  }
  .roll-cell.in-story {
    box-shadow: 0 0 0 2px var(--color-accent);
  }
  .roll-cell img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    pointer-events: none;
    -webkit-user-drag: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .roll-check {
    position: absolute;
    right: 3px;
    bottom: 3px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: var(--color-accent);
    color: var(--color-on-accent);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-shadow: var(--shadow);
  }
</style>
