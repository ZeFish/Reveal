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
          aria-pressed={(recipe?.crop_aspect ?? "original") === a}
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
        style="--slider-value: {(((recipe?.crop_angle ?? 0) + 45) / 90) * 100}%"
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
        aria-pressed={recipe?.flip_h}
        onclick={() => toggleFlip("h")}
        title="Miroir horizontal"
      >
        <Icon name="flip-horizontal" size="14px" />
        <span>Horizontal</span>
      </button>
      <button
        class="action-btn"
        aria-pressed={recipe?.flip_v}
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
    display: flex;
    flex-direction: column;
    gap: var(--space);
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 3);
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

  .reset-btn {
    cursor: pointer;
  }

  .aspect-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-d3);
  }

  .chip {
    cursor: pointer;
    box-sizing: border-box;
    text-align: center;
    padding: var(--space-d3) var(--space-d2);
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: calc(var(--space-d4) * 3);
  }

  .val-mono {
    width: 36px;
    text-align: right;
  }

  .btn-group {
    display: flex;
    gap: var(--space-d2);
  }

  .action-btn {
    cursor: pointer;
    box-sizing: border-box;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-d3);
    padding: var(--space-d2) calc(var(--space-d4) * 3);
  }
</style>
