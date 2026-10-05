<script>
  import CollapsibleGroup from "./controls/CollapsibleGroup.svelte";
  import SliderRow from "./controls/SliderRow.svelte";
  import ToggleRow from "./controls/ToggleRow.svelte";
  import SelectRow from "./controls/SelectRow.svelte";
  import BandMixer from "./controls/BandMixer.svelte";
  import CurveEditor from "../CurveEditor.svelte";
  import { Icon } from "@modules/core";
  import {
    getActiveTarget,
    toDisplay,
    fromDisplay,
    getNeutral,
    formatVal,
  } from "./engineContext.js";

  /**
   * @typedef {Object} Props
   * @property {any} engine
   * @property {any} recipe
   * @property {string} [activeZone]
   * @property {any[]} [films]
   * @property {any[]} [papers]
   * @property {string[]} [luts]
   * @property {any} [defaults]
   * @property {(live?: boolean) => void} [edited]
   * @property {(id: string, index?: number) => void} [resetControl]
   * @property {(stage: string) => void} [addLutLayer]
   * @property {(stage: string, idx: number) => void} [removeLutLayer]
   * @property {(stage: string, idx: number, opacity: number) => void} [updateLutOpacity]
   * @property {(stage: string, idx: number, file: string) => void} [setLutFile]
   */

  /** @type {Props} */
  let {
    engine,
    recipe = $bindable(),
    activeZone = "global",
    films = [],
    papers = [],
    luts = [],
    defaults = null,
    edited = () => {},
    resetControl = () => {},
    addLutLayer = () => {},
    removeLutLayer = () => {},
    updateLutOpacity = () => {},
    setLutFile = () => {},
    ..._rest
  } = $props();

  const isZoneActive = $derived(activeZone && activeZone !== "global");

  function target() {
    return getActiveTarget(recipe, activeZone);
  }

  /** @param {string} id @returns {number} */
  function getSliderVal(id) {
    return Number(target()?.[id] ?? 0);
  }

  /** @param {string} id @param {number} val */
  function setSliderVal(id, val) {
    target()[id] = val;
  }

  /** @param {string} id @param {number} index @returns {number} */
  function getIndexedVal(id, index) {
    return Number(target()?.[id]?.[index] ?? 0);
  }

  /** @param {string} id @param {number} index @param {number} val */
  function setIndexedVal(id, index, val) {
    const t = target();
    if (!Array.isArray(t[id])) t[id] = [];
    t[id][index] = val;
  }

  /** @param {string} id @param {number} [index] */
  function handleReset(id, index) {
    if (isZoneActive) {
      if (index == null) target()[id] = 0;
      else if (Array.isArray(target()[id])) target()[id][index] = 0;
      edited(false);
      return;
    }
    resetControl(id, index);
  }

  /** @param {string} stage */
  function lutListFor(stage) {
    if (isZoneActive) {
      const key = stage === "pre" ? "pre_luts" : "post_luts";
      return Array.isArray(target()[key]) ? target()[key] : [];
    }
    if (stage === "pre") return recipe.rapid_pre_luts?.length ? recipe.rapid_pre_luts : (recipe.pre_luts || []);
    return recipe.rapid_post_luts?.length ? recipe.rapid_post_luts : (recipe.post_luts || []);
  }

  /** @param {string} stage */
  function onAddLut(stage) {
    if (isZoneActive) {
      const key = stage === "pre" ? "pre_luts" : "post_luts";
      if (!Array.isArray(target()[key])) target()[key] = [];
      const first = luts[0] || "";
      target()[key].push({ name: first, opacity: 1.0 });
      edited();
      return;
    }
    addLutLayer(stage);
  }

  /** @param {string} stage @param {number} idx */
  function onRemoveLut(stage, idx) {
    if (isZoneActive) {
      const key = stage === "pre" ? "pre_luts" : "post_luts";
      if (Array.isArray(target()[key])) {
        target()[key].splice(idx, 1);
        edited();
      }
      return;
    }
    removeLutLayer(stage, idx);
  }

  /** @param {string} stage @param {number} idx @param {number} opacity */
  function onUpdateLutOpacity(stage, idx, opacity) {
    if (isZoneActive) {
      const key = stage === "pre" ? "pre_luts" : "post_luts";
      if (target()[key]?.[idx]) {
        target()[key][idx].opacity = opacity;
        edited(true);
      }
      return;
    }
    updateLutOpacity(stage, idx, opacity);
  }

  /** @param {string} stage @param {number} idx @param {string} file */
  function onSetLutFile(stage, idx, file) {
    if (isZoneActive) {
      const key = stage === "pre" ? "pre_luts" : "post_luts";
      if (target()[key]?.[idx]) {
        target()[key][idx].name = file;
        edited();
      }
      return;
    }
    setLutFile(stage, idx, file);
  }

  /** @param {string} optionsType */
  function resolveSelectOptions(optionsType) {
    if (optionsType === "films") return films.map((f) => ({ value: f.name, label: f.label }));
    if (optionsType === "papers") return papers.map((p) => ({ value: p.name, label: p.label }));
    if (optionsType === "agx_looks") {
      return [
        { value: "base", label: "Base Contrast (Standard)" },
        { value: "punchy", label: "Punchy" },
        { value: "golden", label: "Golden (Golden Hour)" },
        { value: "soft", label: "Soft" },
        { value: "bw", label: "Filmic B&W" },
      ];
    }
    return [];
  }
</script>

<div class="generic-engine" data-zone={activeZone}>
  {#if engine && engine.control_groups}
    {#each engine.control_groups as group}
      <CollapsibleGroup label={group.label}>
        {#each group.controls as control}
          {#if control.kind === "toggle"}
            <ToggleRow
              label={control.label}
              checked={Boolean(target()?.[control.id])}
              onChange={(checked) => {
                target()[control.id] = checked;
                edited();
              }}
            />
          {:else if control.kind === "slider"}
            <SliderRow
              label={control.label}
              value={toDisplay(control.id, getSliderVal(control.id))}
              min={control.min}
              max={control.max}
              step={control.step}
              neutral={getNeutral(defaults, activeZone, control.id)}
              formatter={(v) => formatVal(control.id, v)}
              onInput={(v) => {
                setSliderVal(control.id, fromDisplay(control.id, v));
                edited(true);
              }}
              onChange={(v) => {
                setSliderVal(control.id, fromDisplay(control.id, v));
                edited(false);
              }}
              onReset={() => handleReset(control.id)}
            />
          {:else if control.kind === "indexed_slider"}
            <SliderRow
              label={control.label}
              value={getIndexedVal(control.id, control.index)}
              min={control.min}
              max={control.max}
              step={control.step}
              neutral={getNeutral(defaults, activeZone, control.id, control.index)}
              formatter={(v) => formatVal(control.id, v)}
              onInput={(v) => {
                setIndexedVal(control.id, control.index, v);
                edited(true);
              }}
              onChange={(v) => {
                setIndexedVal(control.id, control.index, v);
                edited(false);
              }}
              onReset={() => handleReset(control.id, control.index)}
            />
          {:else if control.kind === "select"}
            <SelectRow
              label={control.label}
              value={recipe?.[control.id] ?? ""}
              options={resolveSelectOptions(control.options_type)}
              onChange={(val) => {
                recipe[control.id] = val;
                edited();
              }}
            />
          {:else if control.kind === "band_mixer"}
            <BandMixer
              label={control.label}
              bands={control.bands}
              channels={control.channels}
              readField={(f) => Number(target()?.[f.id]?.[f.index ?? 0] ?? 0)}
              writeField={(f, v) => {
                const t = target();
                if (!Array.isArray(t[f.id])) t[f.id] = [];
                t[f.id][f.index ?? 0] = v;
              }}
              resetField={(f) => handleReset(f.id, f.index)}
              neutralOf={(id, idx) => getNeutral(defaults, activeZone, id, idx)}
              formatVal={formatVal}
              {edited}
            />
          {:else if control.kind === "curve"}
            {#if activeZone === "shadows"}
              <CurveEditor
                {control}
                bind:recipe={recipe.zone_shadows}
                {edited}
              />
            {:else if activeZone === "midtones"}
              <CurveEditor
                {control}
                bind:recipe={recipe.zone_midtones}
                {edited}
              />
            {:else if activeZone === "highlights"}
              <CurveEditor
                {control}
                bind:recipe={recipe.zone_highlights}
                {edited}
              />
            {:else}
              <CurveEditor
                {control}
                bind:recipe
                {edited}
              />
            {/if}
          {:else if control.kind === "lut_stack"}
            <div class="lut-stack-section">
              <div class="frow sub-bar">
                <span class="lut-subhead">
                  {control.stage === "pre" ? "Pre-Lut" : "Post-Lut"}
                </span>
                <span class="spacer"></span>
                <button type="button" class="outline small add-lut-btn" onclick={() => onAddLut(control.stage)}>
                  + LUT
                </button>
              </div>

              {#each lutListFor(control.stage) as layer, idx}
                <div class="lut-layer-card">
                  <div class="frow layer-row">
                    <select
                      class="panel-select lut-file-pick"
                      value={layer.name}
                      onchange={(e) => onSetLutFile(control.stage, idx, e.currentTarget.value)}
                    >
                      <option value="">(None)</option>
                      {#each luts as name}
                        <option value={name}>{name}</option>
                      {/each}
                    </select>
                    <button
                      type="button"
                      class="ghost icon-btn remove-lut-btn"
                      onclick={() => onRemoveLut(control.stage, idx)}
                      title="Remove this LUT layer"
                    >
                      <Icon name="x" size="9px" />
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
                        oninput={(e) => onUpdateLutOpacity(control.stage, idx, parseFloat(e.currentTarget.value))}
                      />
                      <span class="val mono">{Math.round(layer.opacity * 100)}%</span>
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        {/each}
      </CollapsibleGroup>
    {/each}
  {/if}
</div>

<style>
  .generic-engine {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
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
