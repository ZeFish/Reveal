<script>
  import { Icon, ManualLink } from "@modules/core";
  import Dropdown from "@stnd/ui/Dropdown.svelte";
  import DropdownItem from "@stnd/ui/DropdownItem.svelte";

  /** @typedef {import('@modules/settings/settingsState.svelte.js').Preferences} Preferences */

  /**
   * @typedef {Object} Props
   * @property {Preferences} preferences
   */

  /** @type {Props} */
  let {
    preferences = $bindable(),
  } = $props();

  const AI_PROVIDERS = [
    { id: "anthropic", label: "Anthropic (Claude)", modelPlaceholder: "claude-sonnet-5" },
    { id: "gemini", label: "Google (Gemini)", modelPlaceholder: "gemini-2.5-flash" },
  ];

  const aiCullActive = $derived(Boolean(preferences.ai_cull_mark_story || preferences.ai_cull_export_desktop));

  /** @param {number} delta */
  function adjustTarget(delta) {
    const current = Number(preferences.ai_cull_target) || 24;
    preferences.ai_cull_target = Math.max(1, Math.min(200, current + delta));
  }
</script>

<div class="section-group">
  <div class="section-heading">
    <Icon name="lightning" size="var(--icon-md)" />
    <span>AI CULLING &amp; AUTOMATION</span>
  </div>
  <ManualLink page="cull/ai-cull/#what-leaves-your-mac" label="What AI culling sends, and how it works" />
  <div class="card flush list divided">
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">AUTO-SELECT (STORY)</span>
        <span class="row-desc">Adds the best photos to the quick collection (like the Q key) after every import — no stars, just ready in the preview.</span>
      </div>
      <div class="row-control">
        <input type="checkbox" role="switch" bind:checked={preferences.ai_cull_mark_story} />
      </div>
    </div>
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">AUTO-EXPORT (DESKTOP)</span>
        <span class="row-desc">Develops and exports the best photos to a Desktop folder after every import.</span>
      </div>
      <div class="row-control">
        <input type="checkbox" role="switch" bind:checked={preferences.ai_cull_export_desktop} />
      </div>
    </div>
    <div class="setting-row" aria-disabled={!aiCullActive}>
      <div class="row-meta">
        <span class="row-label">PHOTOS TO KEEP PER SESSION</span>
        <span class="row-desc">Target number of photos kept per date folder during auto-cull — the 24-or-36-exposure roll concept.</span>
      </div>
      <div class="row-control">
        <div class="stepper-group">
          <button type="button" class="stepper-btn" disabled={!aiCullActive || preferences.ai_cull_target <= 1} onclick={() => adjustTarget(-1)} title="Decrease">−</button>
          <input type="number" class="stepper-input mono" min="1" max="200" disabled={!aiCullActive} bind:value={preferences.ai_cull_target} />
          <button type="button" class="stepper-btn" disabled={!aiCullActive || preferences.ai_cull_target >= 200} onclick={() => adjustTarget(1)} title="Increase">+</button>
        </div>
      </div>
    </div>
    <div class="setting-row" aria-disabled={!aiCullActive}>
      <div class="row-meta">
        <span class="row-label">VISION PROVIDER</span>
        <span class="row-desc">Who scores culling candidates and suggests photo tags. Same provider for both — swap it here, not per-feature.</span>
      </div>
      <div class="row-control">
        <Dropdown label="Vision provider" triggerClass="outline small action-pill-btn" align="end">
          {#snippet trigger()}
            <span>{AI_PROVIDERS.find((p) => p.id === preferences.ai_provider)?.label ?? "Anthropic (Claude)"}</span>
            <Icon name="caret-down" size="var(--icon-sm)" />
          {/snippet}
          {#each AI_PROVIDERS as provider}
            <DropdownItem onclick={() => (preferences.ai_provider = provider.id)}>{provider.label}</DropdownItem>
          {/each}
        </Dropdown>
      </div>
    </div>
    <div class="setting-row" aria-disabled={!aiCullActive}>
      <div class="row-meta">
        <span class="row-label">VISION API KEY</span>
        <span class="row-desc">Sends compressed thumbnails to the provider above for ranking and tag suggestions. Billed per API usage.</span>
      </div>
      <div class="row-control">
        <input type="password" class="mono-input" disabled={!aiCullActive} bind:value={preferences.ai_api_key} placeholder={preferences.ai_provider === "gemini" ? "AIza…" : "sk-ant-…"} autocomplete="off" spellcheck="false" />
      </div>
    </div>
    <div class="setting-row" aria-disabled={!aiCullActive}>
      <div class="row-meta">
        <span class="row-label">MODEL</span>
        <span class="row-desc">Leave blank for the provider's default. Must be a valid model id for the selected provider, or requests will fail.</span>
      </div>
      <div class="row-control">
        <input class="mono-input" disabled={!aiCullActive} bind:value={preferences.ai_model} placeholder={AI_PROVIDERS.find((p) => p.id === preferences.ai_provider)?.modelPlaceholder ?? "claude-sonnet-5"} autocomplete="off" spellcheck="false" />
      </div>
    </div>
  </div>
</div>
