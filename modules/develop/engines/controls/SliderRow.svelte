<script>
  import { toPosition, fromPosition, parseTyped, editableText } from "./sliderScale.js";

  /**
   * @typedef {Object} Props
   * @property {string} label
   * @property {number} value
   * @property {number} min
   * @property {number} max
   * @property {number} [step]
   * @property {number} [neutral]
   * @property {number} [hardMin] how low a typed value may go (defaults to `min`; the rail stops at `min`)
   * @property {number} [hardMax] how high a typed value may go (defaults to `max`)
   * @property {(text: string) => number | null} [parse] typed text → value (defaults to the number in the text)
   * @property {(value: number) => string} [editText] the value as it is edited (defaults to the plain number)
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
    hardMin = undefined,
    hardMax = undefined,
    parse = parseTyped,
    editText = undefined,
    disabled = false,
    subParam = false,
    formatter = (v) => (v > 0 ? `+${Math.round(v)}` : `${Math.round(v)}`),
    onInput = () => {},
    onChange = () => {},
    onReset = () => {},
  } = $props();

  let pos = $derived(toPosition(value, min, max, neutral));

  // The value on the right is a field: type a number, Enter or leaving the field keeps it,
  // Escape drops it. A typed value may go past the rail's ends, up to the hard limits.
  let editing = $state(false);
  let draft = $state("");
  /** @type {HTMLInputElement | null} */
  let field = $state(null);

  function startEditing() {
    draft = editText ? editText(value) : editableText(value, step);
    editing = true;
  }

  function commit() {
    if (!editing) return;
    editing = false;
    const typed = parse(draft);
    if (typed === null) return;
    const next = Math.min(hardMax ?? max, Math.max(hardMin ?? min, typed));
    if (next !== value) onChange(next);
  }

  /** @param {KeyboardEvent} e */
  function onFieldKey(e) {
    if (e.key === "Enter") {
      e.preventDefault();
      field?.blur();
    } else if (e.key === "Escape") {
      e.preventDefault();
      editing = false;
      field?.blur();
    }
  }
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

  <input
    class="val mono"
    type="text"
    inputmode="decimal"
    aria-label={`${label} value`}
    {disabled}
    bind:this={field}
    value={editing ? draft : formatter(value)}
    onfocus={(e) => {
      startEditing();
      queueMicrotask(() => e.currentTarget.select());
    }}
    oninput={(e) => (draft = e.currentTarget.value)}
    onkeydown={onFieldKey}
    onblur={commit}
  />
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
    box-shadow: inset var(--stroke-width-lg) 0 0 0 var(--color-border);
  }
  .frow-label {
    width: 6.2rem;
    flex-shrink: 0;
    font-size: 0.76rem;
    color: var(--color-foreground);
    text-align: left;
    white-space: nowrap;
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
    /* A field that looks like the number it was: the frame only shows when it is wanted. */
    background: transparent;
    border-radius: var(--radius);
    padding: 0 2px;
    min-height: 0;
    height: auto;
    box-shadow: none;
  }
  .val:hover {
    box-shadow: var(--shadow-border);
  }
  .val:focus {
    color: var(--color-foreground);
    box-shadow: inset 0 0 0 var(--stroke-width) var(--color-accent);
    outline: none;
  }
</style>
