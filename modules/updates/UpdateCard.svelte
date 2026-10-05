<script>
  // "A new Reveal is ready" — one small card in the notification corner, not a
  // dialog: an update is never urgent, so it must not take the window from
  // someone mid-cull. Three beats: offered (Install / Later / notes),
  // downloading (a bar), and then the app relaunches under it.
  import { updater, installUpdate, dismissUpdate, parseNotes } from "./updater.svelte.js";

  let showNotes = $state(false);
  const groups = $derived(parseNotes(updater.notes));
</script>

{#if updater.hasUpdate}
  <div class="update-card hud" role="status">
    <div class="head">
      <strong>Reveal {updater.version} is ready</strong>
      {#if updater.status === "available"}
        {#if groups.length}
          <button type="button" class="link" onclick={() => (showNotes = !showNotes)}>
            {showNotes ? "Hide notes" : "What's new"}
          </button>
        {/if}
        <button type="button" class="outline small" onclick={dismissUpdate}>Later</button>
        <button type="button" class="small" onclick={installUpdate}>Install &amp; restart</button>
      {:else}
        <span class="status">{updater.status === "ready" ? "Restarting…" : "Downloading…"}</span>
      {/if}
    </div>
    {#if updater.status === "downloading"}
      <progress value={updater.progress < 0 ? undefined : updater.progress} max="1"></progress>
    {/if}
    {#if showNotes && updater.status === "available"}
      <div class="notes">
        {#each groups as group}
          {#if group.heading}<h4>{group.heading}</h4>{/if}
          <ul>
            {#each group.items as item}<li>{item}</li>{/each}
          </ul>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .update-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
    width: min(32rem, calc(100vw - 2rem));
    animation: update-in 200ms ease-out;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
  }
  .head strong {
    flex: 1;
  }
  .status {
    opacity: 0.7;
  }
  .link {
    cursor: pointer;
    opacity: 0.7;
    text-decoration: underline;
  }
  .link:hover {
    opacity: 1;
  }
  progress {
    width: 100%;
  }
  .notes {
    max-height: 40vh;
    overflow-y: auto;
  }
  .notes h4 {
    margin: var(--space-d3) 0 var(--space-d5);
  }
  .notes ul {
    margin: 0;
    padding-left: 1.2em;
  }
  @keyframes update-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
