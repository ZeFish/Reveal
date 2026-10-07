<script>
  import { Icon, ManualLink } from "@modules/core";
  import Alert from "@stnd/ui/Alert.svelte";
  import { invoke } from "@tauri-apps/api/core";

  /** @typedef {import('@modules/settings/settingsState.svelte.js').Preferences} Preferences */

  /**
   * @typedef {Object} Props
   * @property {Preferences} preferences
   */

  /** @type {Props} */
  let {
    preferences = $bindable(),
  } = $props();

  let immichTesting = $state(false);
  let immichMessage = $state("");
  let immichError = $state("");
  let immichDetailsOpen = $state(false);
  const immichConnected = $derived(Boolean(preferences.immich_url && preferences.immich_api_key));

  function disconnectImmich() {
    preferences.immich_url = "";
    preferences.immich_api_key = "";
    preferences.immich_export_enabled = false;
    immichMessage = "";
    immichError = "";
    immichDetailsOpen = false;
  }

  async function testImmich() {
    if (!preferences.immich_url || !preferences.immich_api_key) {
      immichError = "Please specify both the Immich Server URL and API key.";
      immichMessage = "";
      return;
    }
    immichTesting = true;
    immichError = "";
    immichMessage = "";
    try {
      const res = await invoke("test_immich_connection", {
        url: preferences.immich_url,
        apiKey: preferences.immich_api_key,
      });
      const name = res.user?.name || res.user?.email || "User";
      const ver = res.version ? ` (${res.version})` : "";
      immichMessage = `Connected to Immich as ${name}${ver}`;
    } catch (err) {
      immichError = `Connection failed: ${err}`;
    } finally {
      immichTesting = false;
    }
  }
</script>

<div class="section-group">
  <div class="section-heading">
    <Icon name="cloud-arrow-up" size="var(--icon-md)" />
    <span>IMMICH CONNECTION</span>
  </div>
  <ManualLink page="reference/settings/#immich" label="Immich integration in the manual" />
  <div class="card flush list divided">
    {#if immichConnected}
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">ACCOUNT STATUS</span>
          <span class="row-desc">
            {#if immichMessage}
              {immichMessage}
            {:else}
              Connected to Immich at <span class="mono">{preferences.immich_url}</span>.
            {/if}
          </span>
        </div>
        <div class="row-control cache-actions">
          <button
            type="button"
            class="outline small action-pill-btn"
            disabled={immichTesting}
            onclick={testImmich}
          >
            {immichTesting ? "VERIFYING…" : "VERIFY CONNECTION"}
          </button>
          <button
            type="button"
            class="outline small action-pill-btn"
            onclick={disconnectImmich}
          >
            DISCONNECT
          </button>
        </div>
      </div>
      <button
        type="button"
        class="accordion-header"
        onclick={() => (immichDetailsOpen = !immichDetailsOpen)}
        aria-expanded={immichDetailsOpen}
      >
        <div class="row-meta">
          <span class="row-label">CONNECTION SETTINGS</span>
          <span class="row-desc">Server URL and API key</span>
        </div>
        <div class="row-control">
          <Icon name={immichDetailsOpen ? "caret-down" : "caret-right"} size="var(--icon-md)" />
        </div>
      </button>
      {#if immichDetailsOpen}
        <div class="setting-row sub-row">
          <div class="row-meta">
            <span class="row-label">SERVER URL</span>
            <span class="row-desc">Base URL of your Immich instance.</span>
          </div>
          <div class="row-control">
            <input
              class="mono-input"
              bind:value={preferences.immich_url}
              placeholder="http://immich.local:2283"
              spellcheck="false"
            />
          </div>
        </div>
        <div class="setting-row sub-row">
          <div class="row-meta">
            <span class="row-label">API KEY</span>
            <span class="row-desc">Generate in your Immich Account Settings → API Keys.</span>
          </div>
          <div class="row-control">
            <input
              type="password"
              class="mono-input"
              bind:value={preferences.immich_api_key}
              placeholder="Paste your Immich API key"
              spellcheck="false"
            />
          </div>
        </div>
      {/if}
    {:else}
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">SERVER URL</span>
          <span class="row-desc">Base URL of your Immich instance (e.g. <code>http://immich.local:2283</code> or <code>https://immich.yourdomain.com</code>).</span>
        </div>
        <div class="row-control">
          <input
            class="mono-input"
            bind:value={preferences.immich_url}
            placeholder="http://immich.local:2283"
            spellcheck="false"
          />
        </div>
      </div>
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">API KEY</span>
          <span class="row-desc">Generate an API key in your Immich Account Settings → API Keys.</span>
        </div>
        <div class="row-control">
          <input
            type="password"
            class="mono-input"
            bind:value={preferences.immich_api_key}
            placeholder="Paste your Immich API key"
            spellcheck="false"
          />
        </div>
      </div>
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">VERIFY CONNECTION</span>
          <span class="row-desc">Test that Reveal can reach your server and authenticate.</span>
        </div>
        <div class="row-control">
          <button
            type="button"
            class="outline small action-pill-btn"
            disabled={immichTesting || !preferences.immich_url || !preferences.immich_api_key}
            onclick={testImmich}
          >
            {immichTesting ? "TESTING…" : "TEST CONNECTION"}
          </button>
        </div>
      </div>
    {/if}
  </div>
  {#if immichError}<div role="alert"><Alert class="error">{immichError}</Alert></div>{/if}
  {#if immichMessage && !immichConnected}<div role="status"><Alert class="info">{immichMessage}</Alert></div>{/if}
</div>

<div class="section-group">
  <div class="section-heading">
    <Icon name="cloud-arrow-up" size="var(--icon-md)" />
    <span>EXPORT &amp; SYNC</span>
  </div>
  <div class="card flush list divided" class:disabled-card={!immichConnected}>
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">AUTO-UPLOAD ON EXPORT</span>
        <span class="row-desc">
          {#if immichConnected}
            Automatically upload developed JPEGs to your Immich server upon export.
          {:else}
            Connect to your Immich server above to enable automatic uploads.
          {/if}
        </span>
      </div>
      <div class="row-control">
        <input
          type="checkbox"
          role="switch"
          disabled={!immichConnected}
          bind:checked={preferences.immich_export_enabled}
        />
      </div>
    </div>
  </div>
</div>
