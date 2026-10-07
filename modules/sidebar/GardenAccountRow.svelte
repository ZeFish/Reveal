<script>
  import Popover from "@stnd/ui/Popover.svelte";
  import Alert from "@stnd/ui/Alert.svelte";
  import { Icon, ManualLink } from "@modules/core";
  import { invoke } from "@tauri-apps/api/core";

  /**
   * @typedef {Object} Props
   * @property {any} [garden]
   * @property {(key: string) => Promise<any> | void} [onGardenSignIn]
   * @property {() => Promise<any> | void} [onGardenSignOut]
   * @property {(url: string) => void} [onOpenUrl]
   * @property {() => void} [onShowSettings]
   */

  /** @type {Props} */
  let {
    garden = null,
    onGardenSignIn = () => {},
    onGardenSignOut = () => {},
    onOpenUrl = () => {},
    onShowSettings = () => {},
  } = $props();

  let accountOpen = $state(false);
  let pastedKey = $state("");
  let verifying = $state(false);
  /** @type {string | null} */
  let accountError = $state(null);

  const signedIn = $derived(!!garden?.signed_in);

  async function submitKey() {
    if (!pastedKey.trim() || verifying) return;
    verifying = true;
    accountError = null;
    try {
      await onGardenSignIn(pastedKey);
      pastedKey = "";
      accountOpen = false;
    } catch (e) {
      accountError = typeof e === "string" ? e : (/** @type {Error} */ (e)?.message ?? String(e));
    } finally {
      verifying = false;
    }
  }

  async function signOut() {
    try {
      await onGardenSignOut();
    } finally {
      accountOpen = false;
    }
  }
</script>

<div class="account">
  <Popover bind:open={accountOpen} label="Garden account" side="top" onclose={() => { accountError = null; }}>
    {#snippet trigger(/** @type {import('svelte/elements').HTMLButtonAttributes} */ attributes)}
      <button class="account-id" {...attributes}>
        {#if signedIn}
          <span class="account-avatar">{(garden?.username ?? "?").slice(0, 1).toUpperCase()}</span>
          <span class="account-name">{garden?.username}</span>
        {:else}
          <Icon name="stnd-garden" size="var(--icon-lg)" />
          <span>Connect Garden</span>
        {/if}
      </button>
    {/snippet}
    <div class="account-form">
    {#if !signedIn}
      <span class="pop-title">Garden account</span>
      <button class="pop-primary" onclick={async () => { try { onOpenUrl?.(await invoke("garden_begin_connect")); } catch (e) { console.error("garden connect", e); } }}>
        Connect via browser
      </button>
      <span class="pop-or">or</span>
      <span class="pop-hint">Paste an existing API key (Account → API Key on standard.garden).</span>
      <input
        class="pop-key"
        type="password"
        aria-label="Garden API key"
        placeholder="sg_..."
        bind:value={pastedKey}
        onkeydown={(e) => {
          if (e.key === "Enter") submitKey();
        }}
      />
      {#if accountError}
        <div role="alert"><Alert class="error">{accountError}</Alert></div>
      {/if}
      <button class="pop-primary" disabled={!pastedKey.trim() || verifying} onclick={submitKey}>
        {verifying ? "Verifying…" : "Connect"}
      </button>
      <ManualLink page="editorial/garden/" label="Publishing to the Garden" />
    {:else}
      <span class="pop-title">Garden account</span>
      <span class="pop-user">{garden?.username}</span>
      {#if garden?.tier}
        <span class="pop-meta">{garden.tier.toUpperCase()}</span>
      {/if}
      <span class="pop-meta">{garden?.notes_count ?? 0} notes · {garden?.total_views ?? 0} views</span>
      <ManualLink page="editorial/garden/" label="Publishing to the Garden" />
      <div class="pop-divider"></div>
      <button class="pop-danger" onclick={signOut}>Disconnect</button>
    {/if}
    </div>
  </Popover>
  <span class="dir-spacer"></span>
  <button class="ghost icon small" onclick={onShowSettings} title="Reveal settings">
    <Icon name="gear" size="var(--icon-md)" />
  </button>
</div>

<style>
  .account {
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    padding-block-start: var(--space-d2);
  }
  .account-id {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--space-d2);
    min-width: 0;
  }
  .account-avatar {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--color-accent) 15%, transparent);
  }
  .account-name {
    color: var(--color-foreground);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dir-spacer {
    flex: 1;
  }
  .account-form {
    width: 196px;
    display: flex;
    flex-direction: column;
    gap: var(--space-d2);
    padding: calc(var(--space-d4) * 3);
  }
  .pop-title {
    font-weight: bold;
    font-size: var(--scale);
  }
  .pop-or {
    text-align: center;
    font-size: var(--scale-d2);
    opacity: 0.6;
  }
  .pop-hint {
    font-size: var(--scale-d2);
    opacity: 0.7;
    line-height: 1.3;
  }
  .pop-key {
    padding: var(--space-d3);
  }
  .pop-user {
    font-weight: bold;
  }
  .pop-meta {
    font-size: var(--scale-d2);
    opacity: 0.6;
  }
  .pop-primary {
    box-sizing: border-box;
    cursor: pointer;
    text-align: center;
    padding: var(--space-d3) var(--space-d2);
  }
  .pop-primary:disabled {
    cursor: default;
    opacity: 0.35;
  }
  .pop-divider {
    height: 1px;
    background: var(--color-border);
  }
  .pop-danger {
    color: var(--color-red, #ff4444);
    cursor: pointer;
    text-align: center;
    padding: var(--space-d3) var(--space-d2);
  }
</style>
