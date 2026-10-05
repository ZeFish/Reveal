<script>
  import SliderRow from "./SliderRow.svelte";

  /**
   * @typedef {Object} BandField
   * @property {string} id
   * @property {number} [index]
   */

  /**
   * @typedef {Object} Band
   * @property {string} label
   * @property {string} [swatch]
   * @property {BandField[]} fields
   */

  /**
   * @typedef {Object} Channel
   * @property {string} label
   * @property {number} min
   * @property {number} max
   * @property {number} step
   */

  /**
   * @typedef {Object} Props
   * @property {string} label
   * @property {Band[]} bands
   * @property {Channel[]} channels
   * @property {(field: BandField) => number} readField
   * @property {(field: BandField, val: number) => void} writeField
   * @property {(field: BandField) => void} resetField
   * @property {(id: string, index?: number) => number | undefined} neutralOf
   * @property {(id: string, val: number) => string} formatVal
   * @property {(live?: boolean) => void} edited
   */

  /** @type {Props} */
  let {
    label,
    bands = [],
    channels = [],
    readField,
    writeField,
    resetField,
    neutralOf,
    formatVal,
    edited = () => {},
  } = $props();

  let activeBandIndex = $state(0);
  let activeBand = $derived(bands[Math.min(activeBandIndex, bands.length - 1)]);

  /** @param {Band} b */
  function isBandTouched(b) {
    return b.fields.some((/** @type {BandField} */ f) => Math.abs(readField(f)) > 1e-6);
  }
</script>

<div class="band-mixer">
  <div class="bands" role="radiogroup" aria-label={label}>
    {#each bands as b, i}
      <button
        type="button"
        role="radio"
        aria-checked={i === activeBandIndex}
        aria-label={b.label}
        title={b.label}
        class="band"
        class:swatch={!!b.swatch}
        class:active={i === activeBandIndex}
        class:touched={isBandTouched(b)}
        style={b.swatch ? `--swatch: ${b.swatch};` : ""}
        onclick={() => (activeBandIndex = i)}
      >
        {#if !b.swatch}{b.label}{/if}
      </button>
    {/each}
  </div>

  {#if activeBand}
    <div class="band-channels">
      {#each channels as ch, ci}
        {@const field = activeBand.fields[ci]}
        {#if field}
          <SliderRow
            label={ch.label}
            value={readField(field)}
            min={ch.min}
            max={ch.max}
            step={ch.step}
            neutral={neutralOf(field.id, field.index)}
            formatter={(v) => formatVal(field.id, v)}
            onInput={(v) => {
              writeField(field, v);
              edited(true);
            }}
            onChange={(v) => {
              writeField(field, v);
              edited(false);
            }}
            onReset={() => resetField(field)}
          />
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .band-mixer {
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }

  .bands {
    display: flex;
    gap: var(--space-d4);
    background: var(--color-surface);
    padding: var(--space-d4);
    border-radius: var(--radius);
    border: var(--stroke-width) solid var(--color-border);
  }

  .band {
    flex: 1;
    height: 18px;
    border-radius: calc(var(--radius) - 2px);
    border: 1px solid transparent;
    cursor: pointer;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    color: var(--color-muted);
    font-size: 0.65rem;
    transition: all var(--transition-fast);
  }

  .band.swatch {
    background: var(--swatch, transparent);
  }

  .band.active {
    border-color: var(--color-foreground);
    box-shadow: 0 0 0 1px var(--color-foreground);
  }

  .band.touched::after {
    content: "";
    position: absolute;
    bottom: -3px;
    left: 50%;
    transform: translateX(-50%);
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--color-foreground);
  }

  .band-channels {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
  }
</style>
