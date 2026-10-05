import { extractGardenUrl } from "./storyParser.js";

/**
 * Story reactive state using Svelte 5 runes.
 */
export const storyState = $state({
  /** @type {Set<string>} */
  storySet: new Set(),
  /** @type {Set<string>} */
  storyDirs: new Set(),
  filterStory: false,
  /** @type {string | null} */
  liveUrl: null,
  storyContent: "",
  /** @type {{ published: boolean, slug: string, url: string, updated_at: string | null } | null} */
  storyRemote: null,

  get gardenUrl() {
    return extractGardenUrl(this.storyContent) || this.liveUrl;
  },
  get storyPublished() {
    return !!this.storyRemote?.published;
  },
  get publishVerb() {
    return this.storyPublished ? "Update" : "Publish";
  },
  get publishedUrl() {
    const remote = this.storyRemote;
    const gUrl = this.gardenUrl;
    return remote ? (remote.published ? gUrl || remote.url : null) : gUrl;
  },
});
