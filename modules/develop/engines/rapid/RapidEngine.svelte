<script>
  import CollapsibleGroup from "../controls/CollapsibleGroup.svelte";
  import SliderRow from "../controls/SliderRow.svelte";
  import ToggleRow from "../controls/ToggleRow.svelte";
  import SelectRow from "../controls/SelectRow.svelte";
  import BandMixer from "../controls/BandMixer.svelte";
  import CurveEditor from "../../CurveEditor.svelte";
  import { Icon } from "@modules/core";
  import {
    getActiveTarget,
    peekActiveTarget,
    getNeutral,
    formatVal,
  } from "../engineContext.js";

  /**
   * @typedef {Object} Props
   * @property {any} [engine] The engine's declaration from `list_engines`; owns the curve's channels.
   * @property {any} recipe
   * @property {string} [activeZone]
   * @property {string[]} [luts]
   * @property {any} [defaults]
   * @property {(live?: boolean) => void} [edited]
   * @property {(id: string, index?: number) => void} [resetControl]
   * @property {(stage: string) => void} [addLutLayer]
   * @property {(stage: string, idx: number) => void} [removeLutLayer]
   * @property {(stage: string, idx: number, opacity: number) => void} [updateLutOpacity]
   * @property {(stage: string, idx: number, file: string) => void} [setLutFile]
   * @property {() => void} [resetRecipe]
   * @property {(zone: 'shadows' | 'midtones' | 'highlights' | null) => void} [onSetZoneMask]
   */

  /** @type {Props} */
  let {
    engine = null,
    recipe = $bindable(),
    activeZone = $bindable("global"),
    luts = [],
    defaults = null,
    edited = () => {},
    resetControl = () => {},
    addLutLayer = () => {},
    removeLutLayer = () => {},
    updateLutOpacity = () => {},
    setLutFile = () => {},
    resetRecipe = () => {},
    onSetZoneMask = () => {},
  } = $props();

  const isZoneActiveMode = $derived(activeZone && activeZone !== "global");

  /** @type {any} */
  let hoverTimer = null;
  /** @param {'global' | 'shadows' | 'midtones' | 'highlights'} zone */
  function handleZoneMouseEnter(zone) {
    clearTimeout(hoverTimer);
    if (zone === "global") {
      onSetZoneMask(null);
      return;
    }
    hoverTimer = setTimeout(() => {
      onSetZoneMask(zone);
    }, 250);
  }

  function handleZoneMouseLeave() {
    clearTimeout(hoverTimer);
    onSetZoneMask(null);
  }

  /** @param {number[][]} [pts] */
  function isCurveActive(pts) {
    if (!Array.isArray(pts) || pts.length < 2) return false;
    if (pts.length !== 2) return true;
    return (
      Math.abs(pts[0][0]) > 1e-4 ||
      Math.abs(pts[0][1]) > 1e-4 ||
      Math.abs(pts[1][0] - 1) > 1e-4 ||
      Math.abs(pts[1][1] - 1) > 1e-4
    );
  }

  /** @param {any} rec @param {string} zone */
  function isZoneActive(rec, zone) {
    if (!rec || zone === "global") return false;
    const z = rec[`zone_${zone}`];
    if (!z) return false;
    return Boolean(
      Math.abs(z.exposure_ev || 0) > 0.001 ||
      Math.abs(z.contrast || 0) > 0.001 ||
      Math.abs(z.brightness || 0) > 0.001 ||
      Math.abs(z.temperature || 0) > 0.001 ||
      Math.abs(z.tint || 0) > 0.001 ||
      Math.abs(z.saturation || 0) > 0.001 ||
      Math.abs(z.vibrance || 0) > 0.001 ||
      Math.abs(z.whites || 0) > 0.001 ||
      Math.abs(z.highlights || 0) > 0.001 ||
      Math.abs(z.midtones || 0) > 0.001 ||
      Math.abs(z.shadows || 0) > 0.001 ||
      Math.abs(z.blacks || 0) > 0.001 ||
      Math.abs(z.clarity || 0) > 0.001 ||
      Math.abs(z.structure || 0) > 0.001 ||
      Math.abs(z.dehaze || 0) > 0.001 ||
      z.hsl_hue?.some((/** @type {number} */ v) => Math.abs(v) > 0.001) ||
      z.hsl_sat?.some((/** @type {number} */ v) => Math.abs(v) > 0.001) ||
      z.hsl_lum?.some((/** @type {number} */ v) => Math.abs(v) > 0.001) ||
      isCurveActive(z.curve_luma) ||
      isCurveActive(z.curve_r) ||
      isCurveActive(z.curve_g) ||
      isCurveActive(z.curve_b)
    );
  }

  function resetCurrentZone() {
    if (!recipe || activeZone === "global") {
      resetRecipe();
      return;
    }
    const key = `zone_${activeZone}`;
    recipe[key] = {
      exposure_ev: 0,
      contrast: 0,
      brightness: 0,
      temperature: 0,
      tint: 0,
      saturation: 0,
      vibrance: 0,
      whites: 0,
      highlights: 0,
      midtones: 0,
      shadows: 0,
      blacks: 0,
      clarity: 0,
      structure: 0,
      dehaze: 0,
      hsl_hue: Array(8).fill(0),
      hsl_sat: Array(8).fill(0),
      hsl_lum: Array(8).fill(0),
      curve_luma: [[0, 0], [1, 1]],
      curve_r: [[0, 0], [1, 1]],
      curve_g: [[0, 0], [1, 1]],
      curve_b: [[0, 0], [1, 1]],
    };
    edited();
  }

  function target() {
    return getActiveTarget(recipe, activeZone);
  }

  // Display-only reads: never create a missing zone while rendering.
  function peek() {
    return peekActiveTarget(recipe, activeZone);
  }

  /** @param {string} id @returns {number} */
  function getVal(id) {
    return Number(peek()?.[id] ?? 0);
  }

  /** @param {string} id @param {number} val */
  function setVal(id, val) {
    target()[id] = val;
  }

  /** @param {string} id @param {number} [index] */
  function handleReset(id, index) {
    if (isZoneActiveMode) {
      if (index == null) target()[id] = 0;
      else if (Array.isArray(target()[id])) target()[id][index] = 0;
      edited(false);
      return;
    }
    resetControl(id, index);
  }

  // LUT stacks are image-level, so they are global-only (see the Input and Output groups).
  /** @param {string} stage */
  function lutListFor(stage) {
    if (stage === "pre") return recipe.rapid_pre_luts?.length ? recipe.rapid_pre_luts : (recipe.pre_luts || []);
    return recipe.rapid_post_luts?.length ? recipe.rapid_post_luts : (recipe.post_luts || []);
  }

  /** @param {string} stage */
  function onAddLut(stage) {
    addLutLayer(stage);
  }

  /** @param {string} stage @param {number} idx */
  function onRemoveLut(stage, idx) {
    removeLutLayer(stage, idx);
  }

  /** @param {string} stage @param {number} idx @param {number} opacity */
  function onUpdateLutOpacity(stage, idx, opacity) {
    updateLutOpacity(stage, idx, opacity);
  }

  /** @param {string} stage @param {number} idx @param {string} file */
  function onSetLutFile(stage, idx, file) {
    setLutFile(stage, idx, file);
  }

  const AGX_OPTIONS = [
    { value: "base", label: "Base Contrast (Standard)" },
    { value: "punchy", label: "Punchy" },
    { value: "golden", label: "Golden (Golden Hour)" },
    { value: "soft", label: "Soft" },
    { value: "bw", label: "Filmic B&W" },
  ];

  // 8 Color Bands for Band Mixer
  const COLOR_BANDS = [
    { label: "Red", swatch: "#ef4444", fields: [{ id: "band_hue", index: 0 }, { id: "band_sat", index: 0 }, { id: "band_luma", index: 0 }] },
    { label: "Orange", swatch: "#f97316", fields: [{ id: "band_hue", index: 1 }, { id: "band_sat", index: 1 }, { id: "band_luma", index: 1 }] },
    { label: "Yellow", swatch: "#eab308", fields: [{ id: "band_hue", index: 2 }, { id: "band_sat", index: 2 }, { id: "band_luma", index: 2 }] },
    { label: "Green", swatch: "#22c55e", fields: [{ id: "band_hue", index: 3 }, { id: "band_sat", index: 3 }, { id: "band_luma", index: 3 }] },
    { label: "Cyan", swatch: "#06b6d4", fields: [{ id: "band_hue", index: 4 }, { id: "band_sat", index: 4 }, { id: "band_luma", index: 4 }] },
    { label: "Blue", swatch: "#3b82f6", fields: [{ id: "band_hue", index: 5 }, { id: "band_sat", index: 5 }, { id: "band_luma", index: 5 }] },
    { label: "Purple", swatch: "#a855f7", fields: [{ id: "band_hue", index: 6 }, { id: "band_sat", index: 6 }, { id: "band_luma", index: 6 }] },
    { label: "Magenta", swatch: "#ec4899", fields: [{ id: "band_hue", index: 7 }, { id: "band_sat", index: 7 }, { id: "band_luma", index: 7 }] },
  ];

  const BAND_CHANNELS = [
    { label: "Hue", min: -30, max: 30, step: 0.5 },
    { label: "Saturation", min: -100, max: 100, step: 1 },
    { label: "Luminance", min: -100, max: 100, step: 1 },
  ];

  /** @param {{ id: string, index?: number }} field @returns {number} */
  function readBandField(field) {
    const t = peek();
    return Number(t?.[field.id]?.[field.index ?? 0] ?? 0);
  }

  /** @param {{ id: string, index?: number }} field @param {number} val */
  function writeBandField(field, val) {
    const t = target();
    // Always eight bands: a sparse array would serialise as nulls, which Rust rejects.
    if (!Array.isArray(t[field.id]) || t[field.id].length < 8) t[field.id] = Array.from({ length: 8 }, (_, i) => Number(t[field.id]?.[i] ?? 0));
    t[field.id][field.index ?? 0] = val;
  }

  // The curve's channels (luma/R/G/B) are declared by the Rust engine, not here.
  const CURVE_CONTROL = $derived(
    engine?.control_groups
      ?.flatMap((/** @type {any} */ g) => g.controls)
      .find((/** @type {any} */ c) => c.kind === "curve"),
  );
</script>

<div class="rapid-engine" data-zone={activeZone}>
  <!-- Rapid-native Zone Grading Tabs -->
  <div class="zone-tabs-bar">
    <div class="btn-group zone-tabs-group" role="group" aria-label="Working zone">
      <button
        type="button"
        class="zone-btn small"
        title="Global — the whole picture"
        aria-label="Global"
        class:active={activeZone === "global"}
        aria-pressed={activeZone === "global"}
        onclick={() => { activeZone = "global"; onSetZoneMask(null); }}
        onmouseenter={() => handleZoneMouseEnter("global")}
        onmouseleave={handleZoneMouseLeave}
      >
        <Icon name="image" size="var(--icon-lg)" />
      </button>
      <button
        type="button"
        class="zone-btn zone-shadows small"
        title="Shadows"
        aria-label="Shadows"
        class:active={activeZone === "shadows"}
        aria-pressed={activeZone === "shadows"}
        onclick={() => { activeZone = "shadows"; onSetZoneMask(null); }}
        onmouseenter={() => handleZoneMouseEnter("shadows")}
        onmouseleave={handleZoneMouseLeave}
      >
        <Icon name="moon" size="var(--icon-lg)" />
        {#if isZoneActive(recipe, "shadows")}<span class="zone-dot dot-shadows"></span>{/if}
      </button>
      <button
        type="button"
        class="zone-btn zone-midtones small"
        title="Midtones"
        aria-label="Midtones"
        class:active={activeZone === "midtones"}
        aria-pressed={activeZone === "midtones"}
        onclick={() => { activeZone = "midtones"; onSetZoneMask(null); }}
        onmouseenter={() => handleZoneMouseEnter("midtones")}
        onmouseleave={handleZoneMouseLeave}
      >
        <Icon name="circle-half" size="var(--icon-lg)" />
        {#if isZoneActive(recipe, "midtones")}<span class="zone-dot dot-midtones"></span>{/if}
      </button>
      <button
        type="button"
        class="zone-btn zone-highlights small"
        title="Highlights"
        aria-label="Highlights"
        class:active={activeZone === "highlights"}
        aria-pressed={activeZone === "highlights"}
        onclick={() => { activeZone = "highlights"; onSetZoneMask(null); }}
        onmouseenter={() => handleZoneMouseEnter("highlights")}
        onmouseleave={handleZoneMouseLeave}
      >
        <Icon name="sun" size="var(--icon-lg)" />
        {#if isZoneActive(recipe, "highlights")}<span class="zone-dot dot-highlights"></span>{/if}
      </button>
    </div>
  </div>

  <!-- Image-level: input encoding, LUT stacks and the AgX look stay global. A zone
       carries the same adjustments as the global layer, not these. -->
  {#if !isZoneActiveMode}
  <!-- Input (LUT & Encoding) -->
  <CollapsibleGroup label="Input (LUT & Encoding)">
      <ToggleRow
        label="LogC (cinematic)"
        checked={Boolean(recipe?.use_logc)}
        onChange={(checked) => {
          recipe.use_logc = checked;
          edited();
        }}
      />

    <!-- Pre-LUT stack -->
    <div class="lut-stack-section">
      <div class="frow sub-bar">
        <span class="lut-subhead">Pre-Lut</span>
        <span class="spacer"></span>
        <button type="button" class="outline small add-lut-btn" onclick={() => onAddLut("pre")}>+ LUT</button>
      </div>
      {#each lutListFor("pre") as layer, idx}
        <div class="lut-layer-card">
          <div class="frow layer-row">
            <select
              class="panel-select lut-file-pick"
              value={layer.name}
              onchange={(e) => onSetLutFile("pre", idx, e.currentTarget.value)}
            >
              <option value="">(None)</option>
              {#each luts as name}
                <option value={name}>{name}</option>
              {/each}
            </select>
            <button
              type="button"
              class="ghost icon-btn remove-lut-btn"
              onclick={() => onRemoveLut("pre", idx)}
              title="Remove this LUT layer"
            >
              <Icon name="x" size="var(--icon-sm)" />
            </button>
          </div>
          {#if layer.name}
            <div class="frow opacity-row">
              <span class="din opacity-label">Opacity</span>
              <span class="spacer"></span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={layer.opacity}
                style="--slider-value: {layer.opacity * 100}%"
                oninput={(e) => onUpdateLutOpacity("pre", idx, parseFloat(e.currentTarget.value))}
              />
              <span class="val mono">{Math.round(layer.opacity * 100)}%</span>
            </div>
          {/if}
        </div>
      {/each}
    </div>

      <SelectRow
        label="Look AgX"
        value={recipe?.agx_look ?? "base"}
        options={AGX_OPTIONS}
        onChange={(val) => {
          recipe.agx_look = val;
          edited();
        }}
      />
  </CollapsibleGroup>
  {/if}

  <!-- Exposure & Contrast -->
  <CollapsibleGroup label="Exposure & Contrast">
    <SliderRow
      label="Exposure"
      value={getVal("exposure_ev")}
      min={-3}
      max={3}
      step={0.05}
      neutral={getNeutral(defaults, activeZone, "exposure_ev")}
      formatter={(v) => formatVal("exposure_ev", v)}
      onInput={(v) => {
        setVal("exposure_ev", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("exposure_ev", v);
        edited(false);
      }}
      onReset={() => handleReset("exposure_ev")}
    />
    <SliderRow
      label="Brightness"
      value={getVal("brightness")}
      min={-50}
      max={50}
      step={0.5}
      neutral={getNeutral(defaults, activeZone, "brightness")}
      formatter={(v) => formatVal("brightness", v)}
      onInput={(v) => {
        setVal("brightness", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("brightness", v);
        edited(false);
      }}
      onReset={() => handleReset("brightness")}
    />
    <SliderRow
      label="Contrast"
      value={getVal("contrast")}
      min={-0.5}
      max={0.5}
      step={0.01}
      neutral={getNeutral(defaults, activeZone, "contrast")}
      formatter={(v) => formatVal("contrast", v)}
      onInput={(v) => {
        setVal("contrast", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("contrast", v);
        edited(false);
      }}
      onReset={() => handleReset("contrast")}
    />
    <SliderRow
      label="Saturation"
      value={getVal("saturation")}
      min={-1}
      max={0.5}
      step={0.01}
      neutral={getNeutral(defaults, activeZone, "saturation")}
      formatter={(v) => formatVal("saturation", v)}
      onInput={(v) => {
        setVal("saturation", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("saturation", v);
        edited(false);
      }}
      onReset={() => handleReset("saturation")}
    />
    <SliderRow
      label="Vibrance"
      value={getVal("vibrance")}
      min={-50}
      max={50}
      step={0.5}
      neutral={getNeutral(defaults, activeZone, "vibrance")}
      formatter={(v) => formatVal("vibrance", v)}
      onInput={(v) => {
        setVal("vibrance", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("vibrance", v);
        edited(false);
      }}
      onReset={() => handleReset("vibrance")}
    />
    <SliderRow
      label="Highlights"
      value={getVal("highlights")}
      min={-100}
      max={100}
      step={1}
      neutral={getNeutral(defaults, activeZone, "highlights")}
      formatter={(v) => formatVal("highlights", v)}
      onInput={(v) => {
        setVal("highlights", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("highlights", v);
        edited(false);
      }}
      onReset={() => handleReset("highlights")}
    />
    <SliderRow
      label="Shadows"
      value={getVal("shadows")}
      min={-100}
      max={100}
      step={1}
      neutral={getNeutral(defaults, activeZone, "shadows")}
      formatter={(v) => formatVal("shadows", v)}
      onInput={(v) => {
        setVal("shadows", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("shadows", v);
        edited(false);
      }}
      onReset={() => handleReset("shadows")}
    />
    <SliderRow
      label="Whites"
      value={getVal("whites")}
      min={-100}
      max={100}
      step={1}
      neutral={getNeutral(defaults, activeZone, "whites")}
      formatter={(v) => formatVal("whites", v)}
      onInput={(v) => {
        setVal("whites", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("whites", v);
        edited(false);
      }}
      onReset={() => handleReset("whites")}
    />
    <SliderRow
      label="Blacks"
      value={getVal("blacks")}
      min={-100}
      max={100}
      step={1}
      neutral={getNeutral(defaults, activeZone, "blacks")}
      formatter={(v) => formatVal("blacks", v)}
      onInput={(v) => {
        setVal("blacks", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("blacks", v);
        edited(false);
      }}
      onReset={() => handleReset("blacks")}
    />
  </CollapsibleGroup>

  <!-- Tone Curves -->
  <CollapsibleGroup label="Tone Curves">
    {#if CURVE_CONTROL}
    {#if activeZone === "shadows"}
      <CurveEditor
        control={CURVE_CONTROL}
        bind:recipe={recipe.zone_shadows}
        {edited}
      />
    {:else if activeZone === "midtones"}
      <CurveEditor
        control={CURVE_CONTROL}
        bind:recipe={recipe.zone_midtones}
        {edited}
      />
    {:else if activeZone === "highlights"}
      <CurveEditor
        control={CURVE_CONTROL}
        bind:recipe={recipe.zone_highlights}
        {edited}
      />
    {:else}
      <CurveEditor
        control={CURVE_CONTROL}
        bind:recipe
        {edited}
      />
    {/if}
    {/if}
  </CollapsibleGroup>

  <!-- Color -->
  <CollapsibleGroup label="Color">
    <SliderRow
      label="Temp"
      value={getVal("temperature")}
      min={-50}
      max={50}
      step={0.5}
      neutral={getNeutral(defaults, activeZone, "temperature")}
      formatter={(v) => formatVal("temperature", v)}
      onInput={(v) => {
        setVal("temperature", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("temperature", v);
        edited(false);
      }}
      onReset={() => handleReset("temperature")}
    />
    <SliderRow
      label="Tint"
      value={getVal("tint")}
      min={-50}
      max={50}
      step={0.5}
      neutral={getNeutral(defaults, activeZone, "tint")}
      formatter={(v) => formatVal("tint", v)}
      onInput={(v) => {
        setVal("tint", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("tint", v);
        edited(false);
      }}
      onReset={() => handleReset("tint")}
    />

    <!-- Multi-band Mixer -->
    <BandMixer
      label="Color Bands"
      bands={COLOR_BANDS}
      channels={BAND_CHANNELS}
      readField={readBandField}
      writeField={writeBandField}
      resetField={(f) => handleReset(f.id, f.index)}
      neutralOf={(id, idx) => getNeutral(defaults, activeZone, id, idx)}
      formatVal={formatVal}
      {edited}
    />
  </CollapsibleGroup>

  <!-- Detail -->
  <CollapsibleGroup label="Detail">
    <SliderRow
      label="Clarity"
      value={getVal("clarity")}
      min={-40}
      max={60}
      step={0.5}
      neutral={getNeutral(defaults, activeZone, "clarity")}
      formatter={(v) => formatVal("clarity", v)}
      onInput={(v) => {
        setVal("clarity", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("clarity", v);
        edited(false);
      }}
      onReset={() => handleReset("clarity")}
    />
      <SliderRow
        label="Structure"
        value={getVal("structure")}
        min={-30}
        max={50}
        step={0.5}
        neutral={getNeutral(defaults, activeZone, "structure")}
        formatter={(v) => formatVal("structure", v)}
        onInput={(v) => {
          setVal("structure", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("structure", v);
          edited(false);
        }}
        onReset={() => handleReset("structure")}
      />
      <SliderRow
        label="Dehaze"
        value={getVal("dehaze")}
        min={-30}
        max={50}
        step={0.5}
        neutral={getNeutral(defaults, activeZone, "dehaze")}
        formatter={(v) => formatVal("dehaze", v)}
        onInput={(v) => {
          setVal("dehaze", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("dehaze", v);
          edited(false);
        }}
        onReset={() => handleReset("dehaze")}
      />
  </CollapsibleGroup>

  {#if !isZoneActiveMode}
  <!-- Output (Post-LUT) -->
  <CollapsibleGroup label="Output (Post-LUT)">
    <div class="lut-stack-section">
      <div class="frow sub-bar">
        <span class="lut-subhead">Post-Lut</span>
        <span class="spacer"></span>
        <button type="button" class="outline small add-lut-btn" onclick={() => onAddLut("post")}>+ LUT</button>
      </div>
      {#each lutListFor("post") as layer, idx}
        <div class="lut-layer-card">
          <div class="frow layer-row">
            <select
              class="panel-select lut-file-pick"
              value={layer.name}
              onchange={(e) => onSetLutFile("post", idx, e.currentTarget.value)}
            >
              <option value="">(None)</option>
              {#each luts as name}
                <option value={name}>{name}</option>
              {/each}
            </select>
            <button
              type="button"
              class="ghost icon-btn remove-lut-btn"
              onclick={() => onRemoveLut("post", idx)}
              title="Remove this LUT layer"
            >
              <Icon name="x" size="var(--icon-sm)" />
            </button>
          </div>
          {#if layer.name}
            <div class="frow opacity-row">
              <span class="din opacity-label">Opacity</span>
              <span class="spacer"></span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={layer.opacity}
                style="--slider-value: {layer.opacity * 100}%"
                oninput={(e) => onUpdateLutOpacity("post", idx, parseFloat(e.currentTarget.value))}
              />
              <span class="val mono">{Math.round(layer.opacity * 100)}%</span>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </CollapsibleGroup>
  {/if}

  <!-- Digital Effects (Global only) -->
  {#if !isZoneActiveMode}
    <CollapsibleGroup label="Digital Effects">
      <SliderRow
        label="Vignette"
        value={getVal("vignette_amount")}
        min={-2}
        max={2}
        step={0.05}
        neutral={getNeutral(defaults, activeZone, "vignette_amount")}
        formatter={(v) => formatVal("vignette_amount", v)}
        onInput={(v) => {
          setVal("vignette_amount", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("vignette_amount", v);
          edited(false);
        }}
        onReset={() => handleReset("vignette_amount")}
      />
      <SliderRow
        label="Midpoint"
        subParam
        value={getVal("vignette_midpoint")}
        min={0}
        max={1}
        step={0.01}
        neutral={getNeutral(defaults, activeZone, "vignette_midpoint")}
        formatter={(v) => formatVal("vignette_midpoint", v)}
        onInput={(v) => {
          setVal("vignette_midpoint", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("vignette_midpoint", v);
          edited(false);
        }}
        onReset={() => handleReset("vignette_midpoint")}
      />
      <SliderRow
        label="Roundness"
        subParam
        value={getVal("vignette_roundness")}
        min={-1}
        max={1}
        step={0.01}
        neutral={getNeutral(defaults, activeZone, "vignette_roundness")}
        formatter={(v) => formatVal("vignette_roundness", v)}
        onInput={(v) => {
          setVal("vignette_roundness", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("vignette_roundness", v);
          edited(false);
        }}
        onReset={() => handleReset("vignette_roundness")}
      />
      <SliderRow
        label="Feather"
        subParam
        value={getVal("vignette_feather")}
        min={0}
        max={1}
        step={0.01}
        neutral={getNeutral(defaults, activeZone, "vignette_feather")}
        formatter={(v) => formatVal("vignette_feather", v)}
        onInput={(v) => {
          setVal("vignette_feather", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("vignette_feather", v);
          edited(false);
        }}
        onReset={() => handleReset("vignette_feather")}
      />

      <SliderRow
        label="Grain Amount"
        value={getVal("grain_amount")}
        min={0}
        max={100}
        step={1}
        neutral={getNeutral(defaults, activeZone, "grain_amount")}
        formatter={(v) => formatVal("grain_amount", v)}
        onInput={(v) => {
          setVal("grain_amount", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("grain_amount", v);
          edited(false);
        }}
        onReset={() => handleReset("grain_amount")}
      />
      <SliderRow
        label="Roughness"
        subParam
        value={getVal("grain_roughness")}
        min={0}
        max={100}
        step={1}
        neutral={getNeutral(defaults, activeZone, "grain_roughness")}
        formatter={(v) => formatVal("grain_roughness", v)}
        onInput={(v) => {
          setVal("grain_roughness", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("grain_roughness", v);
          edited(false);
        }}
        onReset={() => handleReset("grain_roughness")}
      />

      <SliderRow
        label="Highlight Desat"
        value={getVal("highlight_desat")}
        min={0}
        max={1}
        step={0.01}
        neutral={getNeutral(defaults, activeZone, "highlight_desat")}
        formatter={(v) => formatVal("highlight_desat", v)}
        onInput={(v) => {
          setVal("highlight_desat", v);
          edited(true);
        }}
        onChange={(v) => {
          setVal("highlight_desat", v);
          edited(false);
        }}
        onReset={() => handleReset("highlight_desat")}
      />
    </CollapsibleGroup>
  {/if}

  <!-- Zone Reset action if in a zone -->
  {#if isZoneActiveMode}
    <div class="zone-reset-row">
      <button type="button" class="outline panel-btn half" onclick={resetCurrentZone}>
        Reset {activeZone === "shadows" ? "Shadows" : activeZone === "midtones" ? "Midtones" : "Highlights"}
      </button>
    </div>
  {/if}
</div>

<style>
  .rapid-engine {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
  }

  /* Sticks to the top of the scrolling controls (the panel's scroller is its
     nearest scrolling ancestor), over the panel's own surface so what scrolls
     beneath does not show through. */
  .zone-tabs-bar {
    position: sticky;
    top: 0;
    z-index: 5;
    background: var(--color-surface-light-1);
    padding-top: var(--space-d3);
  }

  .zone-tabs-group {
    display: flex;
    width: 100%;
  }

  .zone-btn {
    flex: 1;
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-height: var(--control-h);
    padding: 0 2px;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    border:0;
  }

  .zone-btn.active {
    font-weight: 600;
  }

  .zone-btn.zone-shadows.active {
    color: var(--color-blue, #3b82f6);
  }

  .zone-btn.zone-midtones.active {
    color: var(--color-green, #10b981);
  }

  .zone-btn.zone-highlights.active {
    color: var(--color-red, #ef4444);
  }

  .zone-dot {
    /* A zone that has been edited: a dot in the corner of its icon. */
    position: absolute;
    top: 4px;
    right: calc(50% - 14px);
    width: 5px;
    height: 5px;
    border-radius: 50%;
    display: inline-block;
  }

  .dot-shadows {
    background-color: var(--color-blue, #3b82f6);
    box-shadow: 0 0 3px var(--color-blue, #3b82f6);
  }

  .dot-midtones {
    background-color: var(--color-green, #10b981);
    box-shadow: 0 0 3px var(--color-green, #10b981);
  }

  .dot-highlights {
    background-color: var(--color-red, #ef4444);
    box-shadow: 0 0 3px var(--color-red, #ef4444);
  }

  .zone-reset-row {
    display: flex;
    margin-top: var(--space-d2);
  }

  .panel-btn {
    padding: var(--space-d3) 0;
  }

  .half {
    flex: 1;
    width: auto;
  }

  .lut-stack-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
  }

  .sub-bar {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
  }

  .lut-subhead {
    font-size: 0.72rem;
    font-family: var(--font-monospace);
    color: var(--color-muted);
  }

  .spacer {
    flex: 1;
  }

  .add-lut-btn {
    font-size: 0.68rem;
    padding: 2px 6px;
    height: 20px;
  }

  .lut-layer-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
    padding: var(--space-d3);
    background: var(--color-surface);
    border-radius: var(--radius);
    border: var(--stroke-width) solid var(--color-border);
  }

  .layer-row {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
  }

  .lut-file-pick {
    flex: 1;
    min-width: 0;
    height: 22px;
    font-size: 0.72rem;
    background: var(--color-surface-light-1);
    border: var(--stroke-width) solid var(--color-border);
    border-radius: var(--radius);
    color: var(--color-foreground);
    padding: 0 var(--space-d3);
  }

  .remove-lut-btn {
    padding: 2px;
    height: 20px;
    width: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }

  .opacity-row {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
  }

  .opacity-label {
    width: 4rem;
    font-size: 0.72rem;
    color: var(--color-muted);
  }

  .val {
    width: 2.8rem;
    text-align: right;
    font-size: 0.72rem;
    color: var(--color-muted);
  }
</style>
