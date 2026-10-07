<script>
  import { Icon, ManualLink } from "@modules/core";
  import Alert from "@stnd/ui/Alert.svelte";
  import { invoke } from "@tauri-apps/api/core";

  /** @typedef {import('../settingsState.svelte.js').Preferences} Preferences */

  /**
   * @typedef {Object} Props
   * @property {Preferences} preferences
   */

  /** @type {Props} */
  let {
    preferences = $bindable(),
  } = $props();

  let googleTesting = $state(false);
  let googleAuthorizing = $state(false);
  let googleMessage = $state("");
  let googleError = $state("");
  let googleDetailsOpen = $state(false);
  const googleConnected = $derived(Boolean(preferences.google_photos_refresh_token));

  async function authorizeGooglePhotos() {
    if (!preferences.google_photos_client_id || !preferences.google_photos_client_secret) {
      googleError = "Please enter both your Client ID and Client Secret.";
      googleMessage = "";
      return;
    }
    googleAuthorizing = true;
    googleError = "";
    googleMessage = "";
    try {
      const res = await invoke("google_photos_start_auth", {
        clientId: preferences.google_photos_client_id,
        clientSecret: preferences.google_photos_client_secret,
      });
      if (res?.refresh_token) {
        preferences.google_photos_refresh_token = res.refresh_token;
        preferences.google_photos_export_enabled = true;
        googleMessage = "Connected to Google Photos successfully!";
      }
    } catch (err) {
      googleError = String(err);
    } finally {
      googleAuthorizing = false;
    }
  }

  async function testGooglePhotos() {
    googleTesting = true;
    googleError = "";
    googleMessage = "";
    try {
      const res = await invoke("google_photos_test_connection");
      googleMessage = res?.message ?? "Connected to Google Photos";
    } catch (err) {
      googleError = `Connection failed: ${err}`;
    } finally {
      googleTesting = false;
    }
  }

  async function disconnectGooglePhotos() {
    try {
      await invoke("google_photos_disconnect");
      preferences.google_photos_refresh_token = "";
      preferences.google_photos_export_enabled = false;
      googleMessage = "Disconnected from Google Photos";
      googleError = "";
      googleDetailsOpen = false;
    } catch (err) {
      googleError = String(err);
    }
  }
</script>

<div class="section-group">
  <div class="section-heading">
    <Icon name="google-photos-logo" size="var(--icon-md)" />
    <span>GOOGLE PHOTOS CONNECTION</span>
  </div>
  <ManualLink page="reference/settings/#google-photos" label="Google Photos setup and permissions in the manual" />
  <div class="card flush list divided">
    {#if googleConnected}
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">ACCOUNT STATUS</span>
          <span class="row-desc">
            Connected to Google Photos. Reveal has permission to add exported photos to your library.
          </span>
        </div>
        <div class="row-control cache-actions">
          <button
            type="button"
            class="outline small action-pill-btn"
            disabled={googleTesting}
            onclick={testGooglePhotos}
          >
            {googleTesting ? "VERIFYING…" : "VERIFY CONNECTION"}
          </button>
          <button
            type="button"
            class="outline small action-pill-btn"
            onclick={disconnectGooglePhotos}
          >
            DISCONNECT
          </button>
        </div>
      </div>
      <button
        type="button"
        class="accordion-header"
        onclick={() => (googleDetailsOpen = !googleDetailsOpen)}
        aria-expanded={googleDetailsOpen}
      >
        <div class="row-meta">
          <span class="row-label">OAUTH CREDENTIALS</span>
          <span class="row-desc">Client ID and Client Secret</span>
        </div>
        <div class="row-control">
          <Icon name={googleDetailsOpen ? "caret-down" : "caret-right"} size="var(--icon-md)" />
        </div>
      </button>
      {#if googleDetailsOpen}
        <div class="setting-row sub-row">
          <div class="row-meta">
            <span class="row-label">CLIENT ID</span>
            <span class="row-desc">OAuth 2.0 Client ID from Google Cloud Console.</span>
          </div>
          <div class="row-control">
            <input
              class="mono-input"
              bind:value={preferences.google_photos_client_id}
              placeholder="xxxx.apps.googleusercontent.com"
              spellcheck="false"
              disabled={googleAuthorizing}
            />
          </div>
        </div>
        <div class="setting-row sub-row">
          <div class="row-meta">
            <span class="row-label">CLIENT SECRET</span>
            <span class="row-desc">OAuth 2.0 Client Secret from Google Cloud Console.</span>
          </div>
          <div class="row-control">
            <input
              type="password"
              class="mono-input"
              bind:value={preferences.google_photos_client_secret}
              placeholder="Paste client secret"
              spellcheck="false"
              disabled={googleAuthorizing}
            />
          </div>
        </div>
      {/if}
    {:else}
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">CLIENT ID</span>
          <span class="row-desc">OAuth 2.0 Client ID for Desktop application from your Google Cloud Console.</span>
        </div>
        <div class="row-control">
          <input
            class="mono-input"
            bind:value={preferences.google_photos_client_id}
            placeholder="xxxx.apps.googleusercontent.com"
            spellcheck="false"
            disabled={googleAuthorizing}
          />
        </div>
      </div>
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">CLIENT SECRET</span>
          <span class="row-desc">OAuth 2.0 Client Secret from your Google Cloud Console.</span>
        </div>
        <div class="row-control">
          <input
            type="password"
            class="mono-input"
            bind:value={preferences.google_photos_client_secret}
            placeholder="Paste client secret"
            spellcheck="false"
            disabled={googleAuthorizing}
          />
        </div>
      </div>
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">ACCOUNT STATUS</span>
          <span class="row-desc">
            Reveal opens your web browser to sign in and grant upload permission to your Google Photos library.
          </span>
        </div>
        <div class="row-control">
          <button
            type="button"
            class="outline small action-pill-btn"
            disabled={googleAuthorizing || !preferences.google_photos_client_id || !preferences.google_photos_client_secret}
            onclick={authorizeGooglePhotos}
          >
            {googleAuthorizing ? "SIGNING IN…" : "CONNECT WITH GOOGLE…"}
          </button>
        </div>
      </div>
    {/if}
  </div>
  {#if googleError}<div role="alert"><Alert class="error">{googleError}</Alert></div>{/if}
  {#if googleMessage}<div role="status"><Alert class="info">{googleMessage}</Alert></div>{/if}
</div>

<div class="section-group">
  <div class="section-heading">
    <Icon name="google-photos-logo" size="var(--icon-md)" />
    <span>EXPORT &amp; SYNC</span>
  </div>
  <div class="card flush list divided" class:disabled-card={!googleConnected}>
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">AUTO-UPLOAD ON EXPORT</span>
        <span class="row-desc">
          {#if googleConnected}
            Automatically upload developed JPEGs to your Google Photos library upon export.
          {:else}
            Connect your Google account above to enable automatic uploads.
          {/if}
        </span>
      </div>
      <div class="row-control">
        <input
          type="checkbox"
          role="switch"
          disabled={!googleConnected}
          bind:checked={preferences.google_photos_export_enabled}
        />
      </div>
    </div>
  </div>
</div>
