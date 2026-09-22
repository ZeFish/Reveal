<script>
  // Tone-curve editor for EngineControl::Curve (see reveal-engine's
  // traits.rs). One tab per channel; each writes a Vec<[f32; 2]> of control
  // points in 0..1 onto the recipe, which the Rust side turns into a
  // 256-entry LUT at render time.
  //
  // The spline drawn here is the SAME monotone cubic Hermite
  // (Fritsch–Carlson) the engine builds its LUT with, ported below rather
  // than approximated with a smoother-looking Catmull-Rom: if the editor
  // drew a curve the renderer doesn't apply, the shape on screen would be a
  // lie exactly where it matters most (steep segments, where Catmull-Rom
  // overshoots and a tone curve must not).
  import Icon from "$lib/components/Icon.svelte";

  let {
    /** @type {{ id: string, label: string, channels: {id: string, label: string, color: string}[] }} */
    control,
    recipe = $bindable(),
    edited = () => {},
  } = $props();

  const IDENTITY = [[0, 0], [1, 1]];
  const SIZE = 100; // SVG user units; CSS scales it to the panel's width

  let activeIndex = $state(0);
  let channel = $derived(control.channels[activeIndex]);
  /** @type {SVGSVGElement | null} */
  let svgEl = $state(null);
  /** @type {number | null} */
  let dragIndex = $state(null);

  /** @param {string} id @returns {number[][]} */
  function pointsOf(id) {
    const v = recipe?.[id];
    return Array.isArray(v) && v.length >= 2 ? v : IDENTITY;
  }
  let points = $derived(pointsOf(channel.id));
  let isIdentity = $derived(
    points.length === 2 &&
      Math.abs(points[0][0]) < 1e-4 &&
      Math.abs(points[0][1]) < 1e-4 &&
      Math.abs(points[1][0] - 1) < 1e-4 &&
      Math.abs(points[1][1] - 1) < 1e-4
  );
  let anyChannelSet = $derived(
    control.channels.some((/** @type {{id: string}} */ c) => {
      const p = pointsOf(c.id);
      return !(
        p.length === 2 &&
        Math.abs(p[0][0]) < 1e-4 &&
        Math.abs(p[0][1]) < 1e-4 &&
        Math.abs(p[1][0] - 1) < 1e-4 &&
        Math.abs(p[1][1] - 1) < 1e-4
      );
    })
  );

  /** @param {number[][]} next @param {boolean} live */
  function commit(next, live) {
    next.sort((a, b) => a[0] - b[0]);
    recipe[channel.id] = next;
    edited(live);
  }

  function resetChannel() {
    commit(IDENTITY.map((p) => [...p]), false);
  }

  /**
   * Monotone cubic Hermite tangents — the JS twin of curves.rs's build_lut.
   * @param {number[]} xs @param {number[]} ys
   */
  function tangents(xs, ys) {
    const n = xs.length;
    const delta = new Array(n - 1);
    for (let i = 0; i < n - 1; i++) delta[i] = (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]);
    const m = new Array(n);
    m[0] = delta[0];
    m[n - 1] = delta[n - 2];
    for (let i = 1; i < n - 1; i++) m[i] = (delta[i - 1] + delta[i]) / 2;
    for (let i = 0; i < n - 1; i++) {
      if (Math.abs(delta[i]) < 1e-9) {
        m[i] = 0;
        m[i + 1] = 0;
        continue;
      }
      const a = m[i] / delta[i];
      const b = m[i + 1] / delta[i];
      const s = a * a + b * b;
      if (s > 9) {
        const t = 3 / Math.sqrt(s);
        m[i] = t * a * delta[i];
        m[i + 1] = t * b * delta[i];
      }
    }
    return m;
  }

  /** @param {number[][]} pts */
  function curvePath(pts) {
    const usable = [...pts].sort((a, b) => a[0] - b[0]);
    if (usable.length < 2) return "";
    const xs = usable.map((p) => p[0]);
    const ys = usable.map((p) => p[1]);
    const m = tangents(xs, ys);

    const STEPS = 64;
    /** @type {string[]} */
    const d = [];
    let seg = 0;
    for (let s = 0; s <= STEPS; s++) {
      const x = s / STEPS;
      let y;
      if (x <= xs[0]) y = ys[0];
      else if (x >= xs[xs.length - 1]) y = ys[ys.length - 1];
      else {
        while (seg + 2 < xs.length && x > xs[seg + 1]) seg++;
        const h = xs[seg + 1] - xs[seg];
        const t = (x - xs[seg]) / h;
        const t2 = t * t;
        const t3 = t2 * t;
        y =
          (2 * t3 - 3 * t2 + 1) * ys[seg] +
          (t3 - 2 * t2 + t) * h * m[seg] +
          (-2 * t3 + 3 * t2) * ys[seg + 1] +
          (t3 - t2) * h * m[seg + 1];
      }
      y = Math.min(1, Math.max(0, y));
      d.push(`${s === 0 ? "M" : "L"}${(x * SIZE).toFixed(2)},${((1 - y) * SIZE).toFixed(2)}`);
    }
    return d.join(" ");
  }

  /** @param {PointerEvent} e @returns {[number, number]} 0..1 curve space */
  function toCurveSpace(e) {
    const rect = /** @type {SVGSVGElement} */ (svgEl).getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width;
    const y = 1 - (e.clientY - rect.top) / rect.height;
    return [Math.min(1, Math.max(0, x)), Math.min(1, Math.max(0, y))];
  }

  /** @param {PointerEvent} e */
  function onSurfacePointerDown(e) {
    if (e.button !== 0) return;
    const [x, y] = toCurveSpace(e);
    const next = points.map((p) => [...p]);
    // A new point lands where you pressed; dragging continues from there, so
    // add-and-place is one gesture rather than click-then-find-it.
    next.push([x, y]);
    next.sort((a, b) => a[0] - b[0]);
    dragIndex = next.findIndex((p) => p[0] === x && p[1] === y);
    /** @type {Element} */ (e.currentTarget).setPointerCapture?.(e.pointerId);
    commit(next, true);
  }

  /** @param {PointerEvent} e @param {number} i */
  function onPointPointerDown(e, i) {
    if (e.button !== 0) return;
    e.stopPropagation();
    dragIndex = i;
    /** @type {Element} */ (e.currentTarget).setPointerCapture?.(e.pointerId);
  }

  /** @param {PointerEvent} e */
  function onPointerMove(e) {
    if (dragIndex === null) return;
    const [x, y] = toCurveSpace(e);
    const next = points.map((p) => [...p]);
    const isFirst = dragIndex === 0;
    const isLast = dragIndex === next.length - 1;
    // The endpoints anchor the curve's domain — they move vertically only,
    // so the curve always spans black-to-white and can't leave a gap the
    // renderer would have to invent a value for.
    next[dragIndex] = [isFirst ? 0 : isLast ? 1 : x, y];
    commit(next, true);
    // commit() re-sorts, so the dragged point may have changed index.
    dragIndex = next.findIndex((p) => p[1] === y && (isFirst || isLast || p[0] === x));
  }

  /** @param {PointerEvent} e */
  function onPointerUp(e) {
    if (dragIndex === null) return;
    dragIndex = null;
    // A settled drag commits for real — same live/settled split every slider
    // in this panel uses (drag = proxy render, release = full render).
    edited(false);
  }

  /** @param {number} i */
  function removePoint(i) {
    if (points.length <= 2) return;
    if (i === 0 || i === points.length - 1) return; // endpoints anchor the domain
    commit(points.filter((_, k) => k !== i).map((p) => [...p]), false);
  }
</script>

<div class="curve-editor">
  <div class="curve-tabs">
    {#each control.channels as ch, i}
      <button
        type="button"
        class="curve-tab din"
        class:active={i === activeIndex}
        style="--tab-color: var({ch.color})"
        onclick={() => (activeIndex = i)}
      >{ch.label}</button>
    {/each}
    <span class="spacer"></span>
    {#if anyChannelSet}
      <button
        type="button"
        class="curve-reset"
        title={isIdentity ? "This channel is already flat" : `Reset the ${channel.label} curve`}
        disabled={isIdentity}
        onclick={resetChannel}
      >
        <Icon name="x" size="9px" />
      </button>
    {/if}
  </div>

  <svg
    bind:this={svgEl}
    class="curve-surface"
    viewBox="0 0 {SIZE} {SIZE}"
    preserveAspectRatio="none"
    role="application"
    aria-label={`${channel.label} tone curve`}
    onpointerdown={onSurfacePointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
  >
    <!-- Quarter grid + the neutral diagonal, so "how far from doing nothing
         is this" is readable at a glance. -->
    {#each [25, 50, 75] as g}
      <line class="grid" x1={g} y1="0" x2={g} y2={SIZE} />
      <line class="grid" x1="0" y1={g} x2={SIZE} y2={g} />
    {/each}
    <line class="diagonal" x1="0" y1={SIZE} x2={SIZE} y2="0" />

    <!-- Every other channel, dimmed, so a red curve isn't edited blind to
         what the luma curve under it is already doing. -->
    {#each control.channels as ch, i}
      {#if i !== activeIndex}
        {@const p = pointsOf(ch.id)}
        {#if !(p.length === 2 && p[0][0] === 0 && p[0][1] === 0 && p[1][0] === 1 && p[1][1] === 1)}
          <path class="curve ghost" style="stroke: var({ch.color})" d={curvePath(p)} />
        {/if}
      {/if}
    {/each}

    <path class="curve" style="stroke: var({channel.color})" d={curvePath(points)} />

    {#each points as p, i}
      <circle
        class="handle"
        class:anchor={i === 0 || i === points.length - 1}
        style="fill: var({channel.color})"
        cx={p[0] * SIZE}
        cy={(1 - p[1]) * SIZE}
        r="2.6"
        role="button"
        tabindex="-1"
        aria-label={`Curve point ${i + 1}`}
        onpointerdown={(e) => onPointPointerDown(e, i)}
        ondblclick={(e) => { e.stopPropagation(); removePoint(i); }}
      />
    {/each}
  </svg>
  <span class="curve-hint">Click to add · drag to shape · double-click a point to remove</span>
</div>

<style>
  .curve-editor {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 2px 0 4px;
  }
  .curve-tabs {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .din {
    font-family: var(--font-header, sans-serif);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .curve-tab {
    -webkit-appearance: none;
    appearance: none;
    background: none;
    border: 0;
    border-radius: 0;
    padding: 1px 4px;
    box-shadow: none;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
    border-bottom: 1px solid transparent;
    cursor: pointer;
  }
  .curve-tab:hover {
    color: var(--color-foreground);
  }
  .curve-tab.active {
    color: var(--tab-color);
    border-bottom-color: var(--tab-color);
  }
  .spacer {
    flex: 1;
  }
  .curve-reset {
    -webkit-appearance: none;
    appearance: none;
    background: none;
    border: 0;
    padding: 0 2px;
    box-shadow: none;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
    cursor: pointer;
  }
  .curve-reset:hover:not(:disabled) {
    color: var(--color-foreground);
  }
  .curve-reset:disabled {
    opacity: 0.25;
    cursor: default;
  }

  .curve-surface {
    width: 100%;
    /* Square-ish: a tone curve is read as a slope against the diagonal, and
       a squashed box makes every slope read steeper than it is. */
    aspect-ratio: 1;
    display: block;
    background: color-mix(in srgb, var(--color-foreground) 5%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-foreground) 12%, transparent);
    border-radius: var(--radius-sm, 3px);
    touch-action: none;
    cursor: crosshair;
  }
  .grid {
    stroke: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    stroke-width: 0.5;
    vector-effect: non-scaling-stroke;
  }
  .diagonal {
    stroke: color-mix(in srgb, var(--color-foreground) 18%, transparent);
    stroke-width: 0.5;
    stroke-dasharray: 2 2;
    vector-effect: non-scaling-stroke;
  }
  .curve {
    fill: none;
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }
  .curve.ghost {
    opacity: 0.25;
    stroke-width: 1;
  }
  .handle {
    stroke: var(--color-background);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
    cursor: grab;
  }
  .handle.anchor {
    opacity: 0.75;
  }
  .curve-hint {
    font-family: var(--font-text, sans-serif);
    font-size: 8.5px;
    color: color-mix(in srgb, var(--color-foreground) 35%, transparent);
  }
</style>
