<script>
  import { Icon } from "@modules/core";
  import Alert from "@stnd/ui/Alert.svelte";
  import AlertDialog from "@stnd/ui/AlertDialog.svelte";
  import { untrack } from "svelte";

  /** @typedef {import('../settingsState.svelte.js').Preferences} Preferences */

  /**
   * @typedef {Object} Props
   * @property {Preferences} preferences
   * @property {boolean} [cacheAvailable]
   * @property {boolean} [saving]
   * @property {() => Promise<{size_bytes: number, limit_bytes: number, in_use_bytes: number}>} [onCacheStatus]
   * @property {() => Promise<{removed_bytes: number, remaining_bytes: number, protected_bytes: number}>} [onCacheClear]
   * @property {() => Promise<{size_bytes: number, photo_count: number, limit_bytes: number}>} [onPreviewCacheStatus]
   * @property {() => Promise<{removed_bytes: number}>} [onPreviewCacheClear]
   */

  /** @type {Props} */
  let {
    preferences = $bindable(),
    cacheAvailable = false,
    saving = false,
    onCacheStatus = async () => ({ size_bytes: 0, limit_bytes: 0, in_use_bytes: 0 }),
    onCacheClear = async () => ({ removed_bytes: 0, remaining_bytes: 0, protected_bytes: 0 }),
    onPreviewCacheStatus = async () => ({ size_bytes: 0, photo_count: 0, limit_bytes: 0 }),
    onPreviewCacheClear = async () => ({ removed_bytes: 0 }),
  } = $props();

  let cacheBusy = $state(false);
  let cacheConfirm = $state(false);
  let cacheError = $state("");
  let cacheMessage = $state("");
  /** @type {{size_bytes: number, limit_bytes: number, in_use_bytes: number} | null} */
  let cacheStatus = $state(null);

  let devCacheBusy = $state(false);
  let devCacheConfirm = $state(false);
  let devCacheError = $state("");
  let devCacheMessage = $state("");
  /** @type {{size_bytes: number, photo_count: number, limit_bytes: number} | null} */
  let devCacheStatus = $state(null);

  /** @param {number} bytes */
  const formatBytes = (bytes) => `${(bytes / 1024 ** 3).toLocaleString("en-CA", { maximumFractionDigits: 2 })} GiB`;

  $effect(() => {
    if (cacheAvailable) untrack(() => {
      preferences.apple_photos_cache_limit_gib ??= 4;
      loadCacheStatus();
    });
  });

  $effect(() => {
    untrack(() => loadDevCacheStatus());
  });

  async function loadDevCacheStatus() {
    if (devCacheBusy) return;
    devCacheBusy = true;
    devCacheError = "";
    try {
      devCacheStatus = await onPreviewCacheStatus();
    } catch (error) {
      devCacheError = `Could not read preview cache usage: ${String(error)}`;
    } finally {
      devCacheBusy = false;
    }
  }

  async function clearDevCache() {
    devCacheConfirm = false;
    if (devCacheBusy) return;
    devCacheBusy = true;
    devCacheError = "";
    devCacheMessage = "";
    try {
      const result = await onPreviewCacheClear();
      devCacheMessage = `Cleared ${formatBytes(result.removed_bytes)}. RAWs redevelop on next view.`;
      devCacheStatus = await onPreviewCacheStatus();
    } catch (error) {
      devCacheError = devCacheMessage
        ? `Cache was cleared, but usage could not be refreshed: ${String(error)}`
        : `Could not clear preview cache: ${String(error)}`;
    } finally {
      devCacheBusy = false;
    }
  }

  async function loadCacheStatus() {
    if (cacheBusy) return;
    cacheBusy = true;
    cacheError = "";
    try {
      cacheStatus = await onCacheStatus();
    } catch (error) {
      cacheError = `Could not read Apple Photos cache usage: ${String(error)}`;
    } finally {
      cacheBusy = false;
    }
  }

  async function clearCache() {
    cacheConfirm = false;
    if (cacheBusy) return;
    cacheBusy = true;
    cacheError = "";
    cacheMessage = "";
    try {
      const result = await onCacheClear();
      cacheMessage = `Cleared ${formatBytes(result.removed_bytes)}; ${formatBytes(result.remaining_bytes)} remains.`
        + (result.protected_bytes > 0 ? ` ${formatBytes(result.protected_bytes)} is in use and was kept. Retry after active work finishes.` : "")
        + " Edits, ratings and captions are unchanged.";
      cacheStatus = await onCacheStatus();
    } catch (error) {
      cacheError = cacheMessage
        ? `Cached copies were cleared, but usage could not be refreshed: ${String(error)}`
        : `Could not complete cache cleanup: ${String(error)}`;
    } finally {
      cacheBusy = false;
    }
  }
</script>

<div class="section-group">
  <div class="section-heading">
    <Icon name="hard-drive" size="var(--icon-md)" />
    <span>DEVELOPED PREVIEWS</span>
  </div>
  <div class="card flush list divided">
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">JPEG RENDER CACHE</span>
        <span class="row-desc">Developed copies cached for instant display in the grid — independent of the source (card, Apple Photos, disk). Clearing forces a redevelop on next display.</span>
        <span class="row-desc" role="status" aria-label="Preview cache usage">
          {#if devCacheStatus}
            {formatBytes(devCacheStatus.size_bytes)} / {formatBytes(devCacheStatus.limit_bytes)} · {devCacheStatus.photo_count.toLocaleString("en-CA")} photos
          {:else}
            {devCacheBusy ? "Reading cache usage…" : "Cache usage unavailable."}
          {/if}
        </span>
      </div>
      <div class="row-control cache-actions">
        <button type="button" class="outline small action-pill-btn" onclick={loadDevCacheStatus} disabled={devCacheBusy}>Refresh</button>
        <button type="button" class="outline small action-pill-btn" onclick={() => { devCacheConfirm = true; }} disabled={devCacheBusy}>Clear cache…</button>
      </div>
    </div>
  </div>
  {#if devCacheError}<div role="alert"><Alert class="error">{devCacheError}</Alert></div>{/if}
  {#if devCacheMessage}<div role="status"><Alert class="info">{devCacheMessage}</Alert></div>{/if}
</div>

{#if cacheAvailable}
  <div class="section-group">
    <div class="section-heading">
      <Icon name="image" size="var(--icon-md)" />
      <span>APPLE PHOTOS CACHE</span>
    </div>
    <div class="card flush list divided">
      <div class="setting-row">
        <div class="row-meta">
          <label class="row-label" for="photos-cache-limit">CACHE LIMIT (GiB)</label>
          <span class="row-desc">1–64 GiB, applied when settings are saved. Files in use remain protected.</span>
        </div>
        <div class="row-control">
          <input id="photos-cache-limit" class="mono-input" type="number" min="1" max="64" step="1"
            bind:value={preferences.apple_photos_cache_limit_gib} disabled={saving} />
        </div>
      </div>
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">CACHED WORKING COPIES</span>
          <span class="row-desc">Clearing is safe for edits, ratings and captions. Originals and previews may need downloading again.</span>
          <span class="row-desc" role="status" aria-label="Cache usage">
            {#if cacheStatus}
              {formatBytes(cacheStatus.size_bytes)} used / {formatBytes(cacheStatus.limit_bytes)} saved limit.
              {formatBytes(cacheStatus.in_use_bytes)} currently in use.
            {:else}
              {cacheBusy ? "Reading cache usage…" : "Cache usage unavailable."}
            {/if}
          </span>
        </div>
        <div class="row-control cache-actions">
          <button type="button" class="outline small action-pill-btn" onclick={loadCacheStatus} disabled={cacheBusy || saving}>Refresh usage</button>
          <button type="button" class="outline small action-pill-btn" onclick={() => { cacheConfirm = true; }} disabled={cacheBusy || saving}>Clear cached copies…</button>
        </div>
      </div>
    </div>
    {#if cacheError}<div role="alert"><Alert class="error">{cacheError}</Alert></div>{/if}
    {#if cacheMessage}<div role="status"><Alert class="info">{cacheMessage}</Alert></div>{/if}
  </div>
{/if}

<AlertDialog bind:open={cacheConfirm} title="Clear Apple Photos cached copies?"
  description="Only downloaded working copies and previews will be cleared. Your edits, ratings and captions are safe. Files currently in use will be kept; other originals may need downloading again."
  confirmLabel="Clear cached copies" cancelLabel="Keep cached copies" intent="danger"
  onconfirm={clearCache} oncancel={() => { cacheConfirm = false; }} />

<AlertDialog bind:open={devCacheConfirm} title="Clear developed preview cache?"
  description="Rendered JPEG copies will be deleted. Your settings, notes and stars are saved elsewhere and unaffected — photos simply redevelop on next display."
  confirmLabel="Clear cache" cancelLabel="Keep cache" intent="danger"
  onconfirm={clearDevCache} oncancel={() => { devCacheConfirm = false; }} />
