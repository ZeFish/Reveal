<script>
  import { Icon, THEME_IDS, DEFAULT_THEME } from "@modules/core";
  import { storyTheme, setStoryTheme, THEMES } from "@modules/story";
  import { invoke } from "@tauri-apps/api/core";

  /**
   * @typedef {Object} Props
   * @property {string | null} [curDir]
   * @property {boolean} [signedIn]
   * @property {boolean} [publishing]
   * @property {boolean} [storyPublished]
   * @property {string | null} [publishStatus]
   * @property {string | null} [gardenUrl]
   * @property {Function} [onPublishStory]
   * @property {Function} [onDevelopStory]
   * @property {Function} [onExportLocalStory]
   * @property {Function} [onOpenUrl]
   */

  /** @type {Props} */
  let {
    curDir = null,
    signedIn = false,
    publishing = false,
    storyPublished = false,
    publishStatus = null,
    gardenUrl = null,
    onPublishStory = () => {},
    onDevelopStory = () => {},
    onExportLocalStory = () => {},
    onOpenUrl = () => {},
  } = $props();

  /** @param {Event & { currentTarget: HTMLSelectElement }} e */
  async function onThemePick(e) {
    const id = e.currentTarget.value || null;
    setStoryTheme(id);
    if (curDir) await invoke("story_set_theme", { dir: curDir, theme: id });
  }
</script>

<div class="story-body">
  <form class="story-theme" onsubmit={(e) => e.preventDefault()}>
    <label for="story-theme-pick">Theme</label>
    <select id="story-theme-pick" value={storyTheme.id ?? ""} onchange={onThemePick} disabled={!curDir}>
      <option value="">Default</option>
      {#each THEMES.filter((t) => t.id === DEFAULT_THEME || THEME_IDS.includes(String(t.id))) as t}
        <option value={String(t.id)}>{t.label}</option>
      {/each}
    </select>
  </form>

  <div class="story-spacer"></div>
  <div class="theme-inner">
    <div class="story-actions">
      {#if signedIn}
        <button class="publish-hero-btn" onclick={() => onPublishStory()} disabled={publishing}>
          {#if publishing}
            <Icon name="arrows-clockwise" size="11px" class="spin" />
            <span>{storyPublished ? "Updating…" : "Publishing…"}</span>
          {:else}
            <Icon name="arrow-square-out" size="11px" />
            <span>{storyPublished ? "Update on Garden" : "Publish to Garden"}</span>
          {/if}
        </button>
      {/if}
      <div class="secondary-actions">
        <button class="action-btn accent" onclick={() => onDevelopStory()} title="Develop every photo in the story">
          <Icon name="sliders-horizontal" size="10px" />
          <span>Develop</span>
        </button>
        <button class="action-btn accent" onclick={() => onExportLocalStory()} disabled={publishing} title="Export the photos locally">
          <Icon name="export" size="10px" />
          <span>Export</span>
        </button>
      </div>
    </div>
    {#if signedIn && gardenUrl}
      <button class="open-page-banner" onclick={() => onOpenUrl(gardenUrl)}>
        <Icon name="check-circle" size="12px" class="banner-check" />
        <span class="banner-text">Live on Garden</span>
        <Icon name="arrow-square-out" size="10px" class="banner-arrow" />
      </button>
    {/if}
    {#if signedIn && publishStatus}
      <p class="publish-status">{publishStatus}</p>
    {/if}
  </div>
</div>

<style>
  .story-body {
    display: flex;
    flex-direction: column;
    gap: var(--space);
    padding: 0 var(--space);
    flex: 1;
    overflow-y: auto;
  }
  .story-spacer {
    flex: 1;
  }
  .theme-inner {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 3);
  }
  .story-actions {
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }
  .publish-hero-btn {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-d2);
    width: 100%;
    padding: var(--space-d2) calc(var(--space-d4) * 3);
    cursor: pointer;
  }
  .publish-hero-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .secondary-actions {
    display: flex;
    gap: var(--space-d3);
  }
  .action-btn.accent {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-d3);
    padding: var(--space-d3) var(--space-d2);
    cursor: pointer;
  }
  .action-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .open-page-banner {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    padding: var(--space-d3) calc(var(--space-d4) * 3);
    cursor: pointer;
  }
  :global(.banner-check) {
    color: var(--color-green);
  }
  :global(.banner-arrow) {
    margin-left: auto;
    opacity: 0.8;
  }
  .publish-status {
    opacity: 0.6;
    margin: 0;
  }
</style>
