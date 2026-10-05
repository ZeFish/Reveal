<script>
  import { toPosition, fromPosition } from "./sliderScale.js";

  /**
   * @typedef {Object} Props
   * @property {string} label
   * @property {number} value
   * @property {number} min
   * @property {number} max
   * @property {number} [step]
   * @property {number} [neutral]
   * @property {boolean} [disabled]
   * @property {boolean} [subParam]
   * @property {(v: number) => string} [formatter]
   * @property {(v: number) => void} [onInput]
   * @property {(v: number) => void} [onChange]
   * @property {() => void} [onReset]
   */

  /** @type {Props} */
  let {
    label,
    value = 0,
    min = -1,
    max = 1,
    step = 0.01,
    neutral = undefined,
    disabled = false,
    subParam = false,
    formatter = (v) => (v > 0 ? `+${Math.round(v)}` : `${Math.round(v)}`),
    onInput = () => {},
    onChange = () => {},
    onReset = () => {},
  } = $props();

  let pos = $derived(toPosition(value, min, max, neutral));
</script>

<div class="frow" class:sub-param={subParam} class:disabled>
  <button
    type="button"
    class="din frow-label reset-label"
    aria-label={`Reset ${label} to default`}
    title={`${label} — double-click or press Enter/Space to reset`}
    {disabled}
    onclick={(e) => { if (e.detail === 0) onReset(); }}
    ondblclick={onReset}
  >{label}</button>

  <input
    type="range"
    aria-label={label}
    {disabled}
    min="0"
    max="1"
    step="any"
    value={pos}
    style="--slider-value: {pos * 100}%"
    oninput={(e) => {
      const next = fromPosition(parseFloat(e.currentTarget.value), min, max, neutral, step);
      onInput(next);
    }}
    onchange={(e) => {
      const next = fromPosition(parseFloat(e.currentTarget.value), min, max, neutral, step);
      onChange(next);
    }}
    ondblclick={onReset}
  />

  <span class="val mono">{formatter(value)}</span>
</div>

<style>
  .frow {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    min-height: 22px;
  }
  .frow.disabled {
    opacity: 0.38;
    pointer-events: none;
  }
  .frow.sub-param {
    padding-left: var(--space-d2);
    border-left: 2px solid var(--color-border);
  }
  .frow-label {
    width: 6.2rem;
    flex-shrink: 0;
    font-size: 0.76rem;
    color: var(--color-foreground);
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .reset-label {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    font-family: inherit;
    transition: color var(--transition-fast);
  }
  .reset-label:hover {
    color: var(--color-accent);
  }
  input[type="range"] {
    flex: 1;
    min-width: 0;
  }
  .val {
    width: 3.2rem;
    flex-shrink: 0;
    font-size: 0.72rem;
    text-align: right;
    color: var(--color-muted);
  }
</style>
