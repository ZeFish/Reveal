<script>
  import { Icon } from "@modules/core";

  let { recipe = $bindable(), edited = () => {} } = $props();

  const aspectPresets = [
    { label: "Original", value: "original" },
    { label: "Libre", value: "free" },
    { label: "1:1", value: "1:1" },
    { label: "4:5", value: "4:5" },
    { label: "3:2", value: "3:2" },
    { label: "4:3", value: "4:3" },
    { label: "16:9", value: "16:9" },
  ];

  const canFlipOrientation = $derived.by(() => {
    const a = recipe?.crop_aspect;
    if (!a || a === "original" || a === "free") return false;
    const parts = a.split(":");
    return parts.length === 2 && parts[0] !== parts[1];
  });

  const isPortrait = $derived.by(() => {
    const a = recipe?.crop_aspect;
    if (!a || !a.includes(":")) return false;
    const parts = a.split(":").map(Number);
    return parts.length === 2 && parts[0] < parts[1];
  });

  /** @param {string} a */
  function setAspect(a) {
    if (!recipe) return;
    recipe.crop_aspect = a;
    if (a === "original") {
      recipe.crop_x = 0;
      recipe.crop_y = 0;
      recipe.crop_w = 1;
      recipe.crop_h = 1;
    }
    edited();
  }

  function toggleAspectOrientation() {
    if (!recipe?.crop_aspect) return;
    const parts = recipe.crop_aspect.split(":");
    if (parts.length === 2 && parts[0] !== parts[1]) {
      recipe.crop_aspect = `${parts[1]}:${parts[0]}`;
      edited();
    }
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
    recipe.crop_x = 0;
    recipe.crop_y = 0;
    recipe.crop_w = 1;
    recipe.crop_h = 1;
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
      <div class="title-actions">
        {#if canFlipOrientation}
          <button
            type="button"
            class="orientation-btn chip"
            onclick={toggleAspectOrientation}
            title="Basculer format Paysage / Portrait ({recipe?.crop_aspect})"
            aria-label="Basculer format Paysage / Portrait"
          >
            <span class="aspect-badge">{recipe?.crop_aspect}</span>
            <span>{isPortrait ? "Portrait" : "Paysage"} ⇄</span>
          </button>
        {/if}
        <button class="reset-btn" onclick={resetCrop} title="Réinitialiser le recadrage">Reset</button>
      </div>
    </div>
    <div class="aspect-grid">
      {#each aspectPresets as preset}
        {@const isActive =
          (recipe?.crop_aspect ?? "original") === preset.value ||
          (preset.value.includes(":") && (recipe?.crop_aspect ?? "") === preset.value.split(":").reverse().join(":"))}
        <button
          class="chip"
          aria-pressed={isActive}
          onclick={() => setAspect(preset.value)}
        >
          {preset.label}
        </button>
      {/each}
    </div>
  </section>

  <div class="hairline"></div>

  <section class="section">
    <div class="section-title">
      <span class="din">Straighten</span>
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
      <span class="din">Orientation & Mirror</span>
    </div>
    <div class="btn-group">
      <button
        class="action-btn"
        aria-pressed={recipe?.flip_h}
        onclick={() => toggleFlip("h")}
        title="Mirror horizontally"
      >
        <Icon name="flip-horizontal" size="14px" />
        <span>Horizontal</span>
      </button>
      <button
        class="action-btn"
        aria-pressed={recipe?.flip_v}
        onclick={() => toggleFlip("v")}
        title="Mirror vertically"
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

  .title-actions {
    display: flex;
    align-items: center;
    gap: var(--space-d2);
  }

  .orientation-btn {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--space-d3);
    padding: var(--space-d4) var(--space-d2);
    font-size: var(--scale-d2);
  }

  .aspect-badge {
    font-weight: bold;
    opacity: 0.8;
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
