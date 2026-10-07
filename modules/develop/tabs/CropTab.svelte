<script>
  import { Icon } from "@modules/core";
  import CollapsibleGroup from "@modules/develop/engines/controls/CollapsibleGroup.svelte";
  import SliderRow from "@modules/develop/engines/controls/SliderRow.svelte";

  let { recipe = $bindable(), edited = () => {} } = $props();

  const aspectPresets = [
    { label: "Original", value: "original" },
    { label: "Free", value: "free" },
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

  /**
   * @param {string | number} val
   * @param {boolean} [live] true while the slider is still being dragged
   */
  function setAngle(val, live = false) {
    if (!recipe) return;
    recipe.crop_angle = Number(val);
    edited(live);
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
  <div class="crop-scroll">
    <CollapsibleGroup label="Proportions">
      <div class="aspect-grid">
        {#each aspectPresets as preset}
          {@const isActive =
            (recipe?.crop_aspect ?? "original") === preset.value ||
            (preset.value.includes(":") && (recipe?.crop_aspect ?? "") === preset.value.split(":").reverse().join(":"))}
          <button class="chip" aria-pressed={isActive} onclick={() => setAspect(preset.value)}>
            {preset.label}
          </button>
        {/each}
        <!-- The eighth cell: turn the frame between landscape and portrait. Greyed out when the
             proportion has no other orientation (Original, Free, 1:1). -->
        <button
          type="button"
          class="chip orientation"
          disabled={!canFlipOrientation}
          onclick={toggleAspectOrientation}
          title={canFlipOrientation
            ? `Switch to ${isPortrait ? "landscape" : "portrait"} (${recipe?.crop_aspect})`
            : "Landscape / portrait — pick a fixed proportion first"}
          aria-label="Switch between landscape and portrait"
        >
          <Icon name="device-rotate" size="var(--icon-lg)" />
        </button>
      </div>
    </CollapsibleGroup>

    <CollapsibleGroup label="Straighten">
      <SliderRow
        label="Angle"
        value={recipe?.crop_angle ?? 0}
        min={-45}
        max={45}
        step={0.5}
        neutral={0}
        formatter={(v) => `${v.toFixed(1)}°`}
        onInput={(v) => setAngle(v, true)}
        onChange={(v) => setAngle(v)}
        onReset={() => setAngle(0)}
      />
    </CollapsibleGroup>

    <CollapsibleGroup label="Orientation & Mirror">
      <div class="btn-group">
        <button class="action-btn" aria-pressed={recipe?.flip_h} onclick={() => toggleFlip("h")} title="Mirror horizontally">
          <Icon name="flip-horizontal" size="var(--icon-lg)" />
          <span>Horizontal</span>
        </button>
        <button class="action-btn" aria-pressed={recipe?.flip_v} onclick={() => toggleFlip("v")} title="Mirror vertically">
          <Icon name="flip-vertical" size="var(--icon-lg)" />
          <span>Vertical</span>
        </button>
      </div>
    </CollapsibleGroup>
  </div>

  <!-- Pinned to the bottom, like Reset / Export on the Dev tab. -->
  <div class="footer">
    <button class="outline panel-btn" onclick={resetCrop} title="Back to the whole frame, no rotation, no mirror">
      Reset crop
    </button>
  </div>
</div>

<style>
  .crop-tab {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-d2);
  }

  .crop-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    /* No scrollbar: it sat on top of the values (same as the Dev tab). */
    scrollbar-width: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }

.crop-scroll > :global(*) {
    /* A group keeps its own height and the area scrolls; left to shrink, they squeezed each
       other flat (the headers and the facts vanished). */
    flex-shrink: 0;
  }

  /* Seven proportions and the orientation switch: two even rows of four. */
  .aspect-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-d3);
    padding-block: var(--space-d3);
  }

  .chip,
  .action-btn {
    cursor: pointer;
    box-sizing: border-box;
    min-height: var(--control-h);
    padding-block: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    text-align: center;
  }
  .chip {
    padding-inline: var(--space-d3);
  }
  .chip:disabled {
    cursor: default;
    opacity: 0.4;
  }

  .btn-group {
    display: flex;
    gap: var(--space-d3);
    padding-block: var(--space-d3);
  }
  .action-btn {
    flex: 1;
    gap: var(--space-d3);
    padding-inline: calc(var(--space-d4) * 3);
  }

  .footer {
    flex-shrink: 0;
    padding: var(--space-d3) 0;
    box-shadow: var(--shadow-border-top);
  }
  .panel-btn {
    width: 100%;
    min-height: var(--control-h);
    padding-block: 0;
  }
</style>
