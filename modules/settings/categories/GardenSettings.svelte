<script>
  import { Icon } from "@modules/core";
  import Alert from "@stnd/ui/Alert.svelte";

  /**
   * @typedef {Object} GardenAccount
   * @property {boolean} signed_in
   * @property {string} [username]
   * @property {string} [tier]
   * @property {number} [notes_count]
   * @property {number} [total_views]
   */

  /**
   * @typedef {Object} Props
   * @property {GardenAccount | null} [gardenAccount]
   * @property {() => Promise<void>} [onGardenSignOut]
   */

  /** @type {Props} */
  let {
    gardenAccount = null,
    onGardenSignOut = async () => {},
  } = $props();

  let gardenSigningOut = $state(false);
  let gardenError = $state("");

  async function signOutOfGarden() {
    if (gardenSigningOut) return;
    gardenSigningOut = true;
    gardenError = "";
    try {
      await onGardenSignOut();
    } catch (error) {
      gardenError = `Could not sign out: ${String(error)}`;
    } finally {
      gardenSigningOut = false;
    }
  }
</script>

<div class="section-group">
  <div class="section-heading">
    <Icon name="user-circle" size="var(--icon-md)" />
    <span>GARDEN ACCOUNT</span>
  </div>
  <div class="card flush list divided">
    {#if gardenAccount?.signed_in}
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">{gardenAccount.username ?? "Connected"}</span>
          <span class="row-desc">Tier: {gardenAccount.tier ?? "—"}</span>
        </div>
        <div class="row-control">
          <button type="button" class="outline small action-pill-btn" onclick={signOutOfGarden} disabled={gardenSigningOut}>
            {gardenSigningOut ? "Signing out…" : "Sign out"}
          </button>
        </div>
      </div>
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">PUBLISHED NOTES</span>
        </div>
        <div class="row-control"><span class="mono">{gardenAccount.notes_count ?? 0}</span></div>
      </div>
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">TOTAL VIEWS</span>
        </div>
        <div class="row-control"><span class="mono">{(gardenAccount.total_views ?? 0).toLocaleString("en-CA")}</span></div>
      </div>
    {:else}
      <div class="setting-row">
        <div class="row-meta">
          <span class="row-label">NOT SIGNED IN</span>
          <span class="row-desc">Sign in from the sidebar (Garden button) to publish your stories.</span>
        </div>
      </div>
    {/if}
  </div>
  {#if gardenError}<div role="alert"><Alert class="error">{gardenError}</Alert></div>{/if}
</div>
