<script>
  import Icon from "$lib/components/Icon.svelte";

  let { recipe = $bindable(), edited = () => {} } = $props();

  const aspects = [
    ["Original", "original"],
    ["Free", "free"],
    ["1:1 Square", "1:1"],
    ["3:2", "3:2"],
    ["4:3", "4:3"],
    ["16:9", "16:9"],
  ];

  /** @param {string} a */
  function setAspect(a) {
    if (!recipe) return;
    recipe.crop_aspect = a;
    edited();
  }

  /** @param {string | number} val */
  function setAngle(val) {
    if (!recipe) return;
    recipe.crop_angle = Number(val);
    edited(true);
  }

  /** @param {"h" | "v"} dir */
  function toggleFlip(dir) {
    if (!recipe) return;
    if (dir === "h") recipe.flip_h = !recipe.flip_h;
    if (dir === "v") recipe.flip_v = !recipe.flip_v;
    edited();
  }

  function resetCrop() {
    if (!recipe) return;
    recipe.crop_aspect = "original";
    recipe.crop_angle = 0;
    recipe.flip_h = false;
    recipe.flip_v = false;
    edited();
  }
</script>

<div class="crop-tab">
  <section class="section">
    <div class="section-title">
      <span class="din">Proportions</span>
      <button class="reset-btn" onclick={resetCrop} title="Reset the crop">Reset</button>
    </div>
    <div class="aspect-grid">
      {#each aspects as [label, a]}
        <button
          class="chip"
          class:active={(recipe?.crop_aspect ?? "original") === a}
          onclick={() => setAspect(a)}
        >
          {label}
        </button>
      {/each}
    </div>
  </section>

  <div class="hairline"></div>

  <section class="section">
    <div class="section-title">
      <span class="din">Redressement</span>
      <button class="reset-btn" onclick={() => setAngle(0)}>0°</button>
    </div>
    <div class="slider-row">
      <input
        type="range"
        min="-45"
        max="45"
        step="0.5"
        value={recipe?.crop_angle ?? 0}
        oninput={(e) => setAngle(e.currentTarget.value)}
      />
      <span class="val-mono">{recipe?.crop_angle ?? 0}°</span>
    </div>
  </section>

  <div class="hairline"></div>

  <section class="section">
    <div class="section-title">
      <span class="din">Orientation & Miroir</span>
    </div>
    <div class="btn-group">
      <button
        class="action-btn"
        class:active={recipe?.flip_h}
        onclick={() => toggleFlip("h")}
        title="Miroir horizontal"
      >
        <Icon name="flip-horizontal" size="14px" />
        <span>Horizontal</span>
      </button>
      <button
        class="action-btn"
        class:active={recipe?.flip_v}
        onclick={() => toggleFlip("v")}
        title="Miroir vertical"
      >
        <Icon name="flip-vertical" size="14px" />
        <span>Vertical</span>
      </button>
    </div>
  </section>
</div>

<style>
  .crop-tab {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .hairline {
    height: 1px;
    background: var(--color-border);
    opacity: 0.6;
  }

  .section-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .din {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }

  .reset-btn {
    all: unset;
    cursor: pointer;
    font-family: var(--font-text, sans-serif);
    font-size: 10px;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
  }
  .reset-btn:hover {
    color: var(--color-foreground);
  }

  .aspect-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
  }

  .chip {
    all: unset;
    cursor: pointer;
    box-sizing: border-box;
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    text-align: center;
    padding: 6px 8px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    color: color-mix(in srgb, var(--color-foreground) 75%, transparent);
    border: 1px solid var(--color-border);
    transition: all var(--duration-instant) var(--ease-soft);
  }
  .chip:hover {
    color: var(--color-foreground);
    border-color: color-mix(in srgb, var(--color-foreground) 25%, transparent);
  }
  .chip.active {
    background: var(--color-surface-high);
    color: var(--color-foreground);
    border-color: var(--color-surface-high);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .val-mono {
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    width: 36px;
    text-align: right;
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
  }

  .btn-group {
    display: flex;
    gap: 8px;
  }

  .action-btn {
    all: unset;
    cursor: pointer;
    box-sizing: border-box;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    padding: 8px 12px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    color: color-mix(in srgb, var(--color-foreground) 75%, transparent);
    border: 1px solid var(--color-border);
    transition: all var(--duration-instant) var(--ease-soft);
  }
  .action-btn:hover {
    color: var(--color-foreground);
    border-color: color-mix(in srgb, var(--color-foreground) 25%, transparent);
  }
  .action-btn.active {
    background: var(--color-surface-high);
    border-color: var(--color-surface-high);
    color: var(--color-foreground);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  }
</style>
