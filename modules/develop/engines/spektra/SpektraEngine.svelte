<script>
  import CollapsibleGroup from "../controls/CollapsibleGroup.svelte";
  import SliderRow from "../controls/SliderRow.svelte";
  import ToggleRow from "../controls/ToggleRow.svelte";
  import SelectRow from "../controls/SelectRow.svelte";
  import {
    toDisplay,
    fromDisplay,
    getNeutral,
    formatVal,
  } from "../engineContext.js";

  /**
   * @typedef {Object} Props
   * @property {any} recipe
   * @property {any[]} [films]
   * @property {any[]} [papers]
   * @property {any} [defaults]
   * @property {(live?: boolean) => void} [edited]
   * @property {(id: string, index?: number) => void} [resetControl]
   */

  /** @type {Props} */
  let {
    recipe = $bindable(),
    films = [],
    papers = [],
    defaults = null,
    edited = () => {},
    resetControl = () => {},
    ..._rest
  } = $props();

  const isPositive = $derived(
    films.find((f) => f.name === recipe?.film)?.film_type === "positive"
  );
  // Only films that ship a family of curves, one per development time, respond to it.
  const hasDevelopmentTimes = $derived(
    films.find((f) => f.name === recipe?.film)?.has_development_times ?? false
  );

  /** @param {string} id @returns {number} */
  function getVal(id) {
    return Number(recipe?.[id] ?? 0);
  }

  /** @param {string} id @param {number} val */
  function setVal(id, val) {
    recipe[id] = val;
  }
</script>

<div class="spektra-engine">
  <!-- General / Exposure -->
  <CollapsibleGroup>
    <ToggleRow
      label="Auto exposure"
      checked={Boolean(recipe?.auto_exposure)}
      onChange={(checked) => {
        recipe.auto_exposure = checked;
        edited();
      }}
    />

    <SliderRow
      label="Exposure"
      value={toDisplay("exposure_ev", getVal("exposure_ev"))}
      min={-3}
      max={3}
      step={0.1}
      neutral={getNeutral(defaults, "global", "exposure_ev")}
      formatter={(v) => formatVal("exposure_ev", v)}
      onInput={(v) => {
        setVal("exposure_ev", fromDisplay("exposure_ev", v));
        edited(true);
      }}
      onChange={(v) => {
        setVal("exposure_ev", fromDisplay("exposure_ev", v));
        edited(false);
      }}
      onReset={() => resetControl("exposure_ev")}
    />
  </CollapsibleGroup>

  <!-- Before the film: the picture's local contrast arranged for it (film_prep.rs). Off by default. -->
  <CollapsibleGroup label="Before the film">
    <SliderRow
      label="Local contrast"
      value={toDisplay("film_prep", getVal("film_prep"))}
      min={0}
      max={100}
      step={5}
      neutral={0}
      formatter={(v) => formatVal("film_prep", v)}
      onInput={(v) => {
        setVal("film_prep", fromDisplay("film_prep", v));
        edited(true);
      }}
      onChange={(v) => {
        setVal("film_prep", fromDisplay("film_prep", v));
        edited(false);
      }}
      onReset={() => resetControl("film_prep")}
    />
  </CollapsibleGroup>

  <!-- Emulsions -->
  <CollapsibleGroup label="Emulsions">
    <SelectRow
      label="Film"
      value={recipe?.film ?? ""}
      options={films.map((f) => ({ value: f.name, label: f.label }))}
      onChange={(val) => {
        recipe.film = val;
        edited();
      }}
    />
    <SelectRow
      label="Paper"
      value={recipe?.paper ?? ""}
      disabled={isPositive}
      options={papers.map((p) => ({ value: p.name, label: p.label }))}
      onChange={(val) => {
        recipe.paper = val;
        edited();
      }}
    />
    <SliderRow
      label="Format (mm)"
      value={getVal("film_format_mm")}
      min={4}
      max={120}
      step={1}
      neutral={getNeutral(defaults, "global", "film_format_mm")}
      formatter={(v) => formatVal("film_format_mm", v)}
      onInput={(v) => {
        setVal("film_format_mm", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("film_format_mm", v);
        edited(false);
      }}
      onReset={() => resetControl("film_format_mm")}
    />
  </CollapsibleGroup>

  <!-- Chemistry (B&W only) -->
  <CollapsibleGroup label="Chemistry">
    <SliderRow
      label="Duration"
      disabled={!hasDevelopmentTimes}
      value={getVal("development_time_min")}
      min={0}
      max={20}
      step={0.5}
      neutral={getNeutral(defaults, "global", "development_time_min")}
      formatter={(v) => formatVal("development_time_min", v)}
      onInput={(v) => {
        setVal("development_time_min", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("development_time_min", v);
        edited(false);
      }}
      onReset={() => resetControl("development_time_min")}
    />
  </CollapsibleGroup>

  <!-- Prints (Negatives only) -->
  <CollapsibleGroup label="Prints">
    <SliderRow
      label="Exposure"
      disabled={isPositive}
      value={toDisplay("print_exposure_ev", getVal("print_exposure_ev"))}
      min={-3}
      max={3}
      step={0.1}
      neutral={getNeutral(defaults, "global", "print_exposure_ev")}
      formatter={(v) => formatVal("exposure_ev", v)}
      onInput={(v) => {
        setVal("print_exposure_ev", fromDisplay("print_exposure_ev", v));
        edited(true);
      }}
      onChange={(v) => {
        setVal("print_exposure_ev", fromDisplay("print_exposure_ev", v));
        edited(false);
      }}
      onReset={() => resetControl("print_exposure_ev")}
    />
    <SliderRow
      label="Grade"
      disabled={isPositive}
      value={getVal("contrast_grade")}
      min={0}
      max={5}
      step={0.1}
      neutral={getNeutral(defaults, "global", "contrast_grade")}
      formatter={(v) => formatVal("contrast_grade", v)}
      onInput={(v) => {
        setVal("contrast_grade", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("contrast_grade", v);
        edited(false);
      }}
      onReset={() => resetControl("contrast_grade")}
    />
    <SliderRow
      label="Gamma"
      disabled={isPositive}
      value={getVal("density_gamma")}
      min={-0.5}
      max={0.5}
      step={0.01}
      neutral={getNeutral(defaults, "global", "density_gamma")}
      formatter={(v) => formatVal("density_gamma", v)}
      onInput={(v) => {
        setVal("density_gamma", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("density_gamma", v);
        edited(false);
      }}
      onReset={() => resetControl("density_gamma")}
    />
      <SliderRow
      label="Y Filter"
      value={getVal("y_shift")}
      min={-10}
      max={10}
      step={1}
      neutral={getNeutral(defaults, "global", "y_shift")}
      formatter={(v) => formatVal("y_shift", v)}
      onInput={(v) => {
        setVal("y_shift", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("y_shift", v);
        edited(false);
      }}
      onReset={() => resetControl("y_shift")}
    />
    <SliderRow
      label="M Filter"
      value={getVal("m_shift")}
      min={-10}
      max={10}
      step={1}
      neutral={getNeutral(defaults, "global", "m_shift")}
      formatter={(v) => formatVal("m_shift", v)}
      onInput={(v) => {
        setVal("m_shift", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("m_shift", v);
        edited(false);
      }}
      onReset={() => resetControl("m_shift")}
    />
  </CollapsibleGroup>

  <!-- Film Effects -->
  <CollapsibleGroup label="Film Effects">
    <SliderRow
      label="Halation"
      value={getVal("halation")}
      min={0}
      max={1}
      step={0.05}
      neutral={getNeutral(defaults, "global", "halation")}
      formatter={(v) => formatVal("halation", v)}
      onInput={(v) => {
        setVal("halation", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("halation", v);
        edited(false);
      }}
      onReset={() => resetControl("halation")}
    />
    <SliderRow
      label="Halation Size"
      value={getVal("halation_size")}
      min={0.5}
      max={1.5}
      step={0.05}
      neutral={getNeutral(defaults, "global", "halation_size")}
      formatter={(v) => formatVal("halation_size", v)}
      onInput={(v) => {
        setVal("halation_size", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("halation_size", v);
        edited(false);
      }}
      onReset={() => resetControl("halation_size")}
    />
    <SliderRow
      label="Diffusion"
      value={getVal("diffusion")}
      min={0}
      max={1.5}
      step={0.05}
      neutral={getNeutral(defaults, "global", "diffusion")}
      formatter={(v) => formatVal("diffusion", v)}
      onInput={(v) => {
        setVal("diffusion", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("diffusion", v);
        edited(false);
      }}
      onReset={() => resetControl("diffusion")}
    />
    <SliderRow
      label="Grain"
      value={getVal("grain")}
      min={0}
      max={1}
      step={0.05}
      neutral={getNeutral(defaults, "global", "grain")}
      formatter={(v) => formatVal("grain", v)}
      onInput={(v) => {
        setVal("grain", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("grain", v);
        edited(false);
      }}
      onReset={() => resetControl("grain")}
    />
    <SliderRow
      label="Sharpen"
      value={getVal("sharpen")}
      min={0}
      max={1}
      step={0.05}
      neutral={getNeutral(defaults, "global", "sharpen")}
      formatter={(v) => formatVal("sharpen", v)}
      onInput={(v) => {
        setVal("sharpen", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("sharpen", v);
        edited(false);
      }}
      onReset={() => resetControl("sharpen")}
    />

    <ToggleRow
      label="Glare"
      checked={Boolean(recipe?.glare)}
      onChange={(checked) => {
        recipe.glare = checked;
        edited();
      }}
    />
    <SliderRow
      label="Glare Amount"
      subParam
      disabled={!recipe?.glare}
      value={getVal("glare_percent")}
      min={0}
      max={1}
      step={0.01}
      neutral={getNeutral(defaults, "global", "glare_percent")}
      formatter={(v) => formatVal("glare_percent", v)}
      onInput={(v) => {
        setVal("glare_percent", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("glare_percent", v);
        edited(false);
      }}
      onReset={() => resetControl("glare_percent")}
    />
    <SliderRow
      label="Glare Roughness"
      subParam
      disabled={!recipe?.glare}
      value={getVal("glare_roughness")}
      min={0}
      max={1}
      step={0.01}
      neutral={getNeutral(defaults, "global", "glare_roughness")}
      formatter={(v) => formatVal("glare_roughness", v)}
      onInput={(v) => {
        setVal("glare_roughness", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("glare_roughness", v);
        edited(false);
      }}
      onReset={() => resetControl("glare_roughness")}
    />
    <SliderRow
      label="Glare Blur"
      subParam
      disabled={!recipe?.glare}
      value={getVal("glare_blur")}
      min={0}
      max={1}
      step={0.01}
      neutral={getNeutral(defaults, "global", "glare_blur")}
      formatter={(v) => formatVal("glare_blur", v)}
      onInput={(v) => {
        setVal("glare_blur", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("glare_blur", v);
        edited(false);
      }}
      onReset={() => resetControl("glare_blur")}
    />

    <SliderRow
      label="Preflash"
      value={getVal("preflash_exposure")}
      min={0}
      max={1}
      step={0.01}
      neutral={getNeutral(defaults, "global", "preflash_exposure")}
      formatter={(v) => formatVal("preflash_exposure", v)}
      onInput={(v) => {
        setVal("preflash_exposure", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("preflash_exposure", v);
        edited(false);
      }}
      onReset={() => resetControl("preflash_exposure")}
    />
    <SliderRow
      label="Preflash Y"
      subParam
      disabled={!(getVal("preflash_exposure") > 0)}
      value={getVal("preflash_y_shift")}
      min={-0.5}
      max={0.5}
      step={0.01}
      neutral={getNeutral(defaults, "global", "preflash_y_shift")}
      formatter={(v) => formatVal("contrast", v)}
      onInput={(v) => {
        setVal("preflash_y_shift", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("preflash_y_shift", v);
        edited(false);
      }}
      onReset={() => resetControl("preflash_y_shift")}
    />
    <SliderRow
      label="Preflash M"
      subParam
      disabled={!(getVal("preflash_exposure") > 0)}
      value={getVal("preflash_m_shift")}
      min={-0.5}
      max={0.5}
      step={0.01}
      neutral={getNeutral(defaults, "global", "preflash_m_shift")}
      formatter={(v) => formatVal("contrast", v)}
      onInput={(v) => {
        setVal("preflash_m_shift", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("preflash_m_shift", v);
        edited(false);
      }}
      onReset={() => resetControl("preflash_m_shift")}
    />

    <ToggleRow
      label="DIR Couplers"
      checked={Boolean(recipe?.dir_couplers_active)}
      onChange={(checked) => {
        recipe.dir_couplers_active = checked;
        edited();
      }}
    />
    <SliderRow
      label="Amount"
      subParam
      disabled={!recipe?.dir_couplers_active}
      value={getVal("dir_couplers_amount")}
      min={0}
      max={1}
      step={0.01}
      neutral={getNeutral(defaults, "global", "dir_couplers_amount")}
      formatter={(v) => formatVal("dir_couplers_amount", v)}
      onInput={(v) => {
        setVal("dir_couplers_amount", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("dir_couplers_amount", v);
        edited(false);
      }}
      onReset={() => resetControl("dir_couplers_amount")}
    />
    <SliderRow
      label="Diffusion"
      subParam
      disabled={!recipe?.dir_couplers_active}
      value={getVal("dir_couplers_diffusion_size")}
      min={0}
      max={1}
      step={0.01}
      neutral={getNeutral(defaults, "global", "dir_couplers_diffusion_size")}
      formatter={(v) => formatVal("dir_couplers_diffusion_size", v)}
      onInput={(v) => {
        setVal("dir_couplers_diffusion_size", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("dir_couplers_diffusion_size", v);
        edited(false);
      }}
      onReset={() => resetControl("dir_couplers_diffusion_size")}
    />
    <SliderRow
      label="Tail"
      subParam
      disabled={!recipe?.dir_couplers_active}
      value={getVal("dir_couplers_diffusion_tail")}
      min={0}
      max={1}
      step={0.01}
      neutral={getNeutral(defaults, "global", "dir_couplers_diffusion_tail")}
      formatter={(v) => formatVal("dir_couplers_diffusion_tail", v)}
      onInput={(v) => {
        setVal("dir_couplers_diffusion_tail", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("dir_couplers_diffusion_tail", v);
        edited(false);
      }}
      onReset={() => resetControl("dir_couplers_diffusion_tail")}
    />
    <SliderRow
      label="Tail Weight"
      subParam
      disabled={!recipe?.dir_couplers_active}
      value={getVal("dir_couplers_tail_weight")}
      min={0}
      max={1}
      step={0.01}
      neutral={getNeutral(defaults, "global", "dir_couplers_tail_weight")}
      formatter={(v) => formatVal("dir_couplers_tail_weight", v)}
      onInput={(v) => {
        setVal("dir_couplers_tail_weight", v);
        edited(true);
      }}
      onChange={(v) => {
        setVal("dir_couplers_tail_weight", v);
        edited(false);
      }}
      onReset={() => resetControl("dir_couplers_tail_weight")}
    />
  </CollapsibleGroup>
</div>

<style>
  .spektra-engine {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
  }
</style>
