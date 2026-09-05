<script>
  /**
   * @typedef {Object} Props
   * @property {*} [photoMenu]
   * @property {Set<string>} [selectedPaths]
   * @property {any[]} [installedEditors]
   * @property {*} [copiedRecipe]
   * @property {Set<string>} [storySet]
   * @property {(s: string) => string} [stem]
   * @property {() => void} [onClose]
   * @property {(p: string) => void} [onOpenPhoto]
   * @property {(p: string) => void} [onOpenPreview]
   * @property {(p: string) => void} [onRevealInFinder]
   * @property {(p: string, app: string) => void} [onOpenInEditor]
   * @property {(p: string) => void} [onCopyImage]
   * @property {() => void} [onCopySettings]
   * @property {() => void} [onPasteSettings]
   * @property {(n: number) => void} [onRate]
   * @property {(p: string) => void} [onToggleStory]
   * @property {() => void} [onExportSelection]
   * @property {string | null} [gardenUrl]
   * @property {() => void} [onCull]
   * @property {() => void} [onPublishStory]
   * @property {() => void} [onOpenGardenUrl]
   * @property {(p: string) => void} [onDevelopToVault]
   */

  /** @type {Props} */
  let {
    photoMenu = null,
    selectedPaths = new Set(),
    installedEditors = [],
    copiedRecipe = null,
    storySet = new Set(),
    gardenUrl = null,
    stem = (/** @type {string} */ s) => s,
    onClose = () => {},
    onOpenPhoto = (/** @type {string} */ p) => {},
    onOpenPreview = (/** @type {string} */ p) => {},
    onRevealInFinder = (/** @type {string} */ p) => {},
    onOpenInEditor = (/** @type {string} */ p, /** @type {string} */ app) => {},
    onCopyImage = (/** @type {string} */ p) => {},
    onCopySettings = () => {},
    onPasteSettings = () => {},
    onRate = (/** @type {number} */ n) => {},
    onToggleStory = (/** @type {string} */ p) => {},
    onExportSelection = () => {},
    onDevelopToVault = (/** @type {string} */ p) => {},
    onCull = undefined,
    onPublishStory = undefined,
    onOpenGardenUrl = undefined,
  } = $props();
</script>

{#if photoMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="context-backdrop" onclick={onClose} role="presentation"></div>
  <div
    class="photo-context-menu std-menu-content m-0"
    style="left: {photoMenu.x}px; top: {photoMenu.y}px"
    role="menu"
    tabindex="-1"
    oncontextmenu={(event) => event.preventDefault()}
  >
    <div class="std-menu-label">
      {selectedPaths.size > 1 ? `${selectedPaths.size} photos sélectionnées` : photoMenu.frame.name}
    </div>
    <div class="std-menu-separator"></div>
    <button class="std-menu-item" role="menuitem" onclick={() => { onOpenPhoto(photoMenu.frame.path); onClose(); }}>
      <span class="item-label">Développer</span>
      <kbd class="item-shortcut">d</kbd>
    </button>
    <button class="std-menu-item" role="menuitem" onclick={() => { onOpenPreview(photoMenu.frame.path); onClose(); }}>
      <span class="item-label">Aperçu (Quick Look)</span>
      <kbd class="item-shortcut">Espace</kbd>
    </button>
    <button class="std-menu-item" role="menuitem" onclick={() => { onRevealInFinder(photoMenu.frame.path); onClose(); }}>
      <span class="item-label">Afficher dans le Finder</span>
    </button>
    {#if installedEditors.length}
      <div class="context-submenu">
        <button class="std-menu-item" role="menuitem" aria-haspopup="menu">
          <span class="item-label">Ouvrir avec…</span>
          <span class="submenu-arrow">›</span>
        </button>
        <div
          class="context-submenu-panel std-menu-content m-0"
          class:submenu-left={photoMenu.submenuLeft}
          role="menu"
          tabindex="-1"
        >
          {#each installedEditors as [name, appPath]}
            <button class="std-menu-item" role="menuitem" onclick={() => { onOpenInEditor(photoMenu.frame.path, appPath); onClose(); }}><span class="item-label">{name}</span></button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="std-menu-separator"></div>

    <button class="std-menu-item" role="menuitem" onclick={() => { onCopyImage(photoMenu.frame.path); onClose(); }}>
      <span class="item-label">Copier l'image</span>
      <kbd class="item-shortcut">⌘C</kbd>
    </button>
    <button class="std-menu-item" role="menuitem" onclick={() => { onCopySettings(); onClose(); }}>
      <span class="item-label">Copier les réglages</span>
      <kbd class="item-shortcut">c</kbd>
    </button>
    <button
      class="std-menu-item"
      role="menuitem"
      disabled={!copiedRecipe}
      class:disabled={!copiedRecipe}
      onclick={() => { onPasteSettings(); onClose(); }}
    >
      <span class="item-label">Coller les réglages</span>
      <kbd class="item-shortcut">v</kbd>
    </button>

    <div class="std-menu-separator"></div>

    <div class="context-rating-row">
      <span class="context-rating-label">Note</span>
      <div class="context-stars">
        {#each [1, 2, 3, 4, 5] as star}
          <button
            class="star-btn"
            class:active={(photoMenu.frame.rating || 0) >= star}
            onclick={() => { onRate(star); onClose(); }}
            title="{star} étoile{star > 1 ? 's' : ''}"
          >★</button>
        {/each}
        {#if photoMenu.frame.rating}
          <button
            class="star-clear"
            onclick={() => { onRate(0); onClose(); }}
            title="Retirer la note"
          >✕</button>
        {/if}
      </div>
    </div>

    <button class="std-menu-item" role="menuitem" onclick={() => { onToggleStory(photoMenu.frame.path); onClose(); }}>
      <span class="item-label">{storySet.has(stem(photoMenu.frame.name)) ? "Retirer de la collection" : "Ajouter à la collection"}</span>
      <kbd class="item-shortcut">q</kbd>
    </button>

    <div class="std-menu-separator"></div>

    <button class="std-menu-item" role="menuitem" onclick={() => { onExportSelection(); onClose(); }}>
      <span class="item-label">Exporter la sélection…</span>
      <kbd class="item-shortcut">r</kbd>
    </button>
    <button class="std-menu-item" role="menuitem" onclick={() => { onDevelopToVault(photoMenu.frame.path); onClose(); }}>
      <span class="item-label">{selectedPaths.size > 1 ? "Ajouter la sélection à la note du jour" : "Ajouter à la note du jour (Obsidian)"}</span>
    </button>

    {#if onCull || (storySet.size > 0 && onPublishStory) || (gardenUrl && onOpenGardenUrl)}
      <div class="std-menu-separator"></div>
    {/if}

    {#if onCull}
      <button class="std-menu-item" role="menuitem" onclick={() => { onCull(); onClose(); }}>
        <span class="item-label">Culling IA</span>
      </button>
    {/if}
    {#if storySet.size > 0 && onPublishStory}
      <button class="std-menu-item" role="menuitem" onclick={() => { onPublishStory(); onClose(); }}>
        <span class="item-label">Publier la collection ({storySet.size})</span>
      </button>
    {/if}
    {#if gardenUrl && onOpenGardenUrl}
      <button class="std-menu-item" role="menuitem" onclick={() => { onOpenGardenUrl(); onClose(); }}>
        <span class="item-label">Ouvrir sur le web</span>
      </button>
    {/if}
  </div>
{/if}

<style>
  .context-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9998;
    background: transparent;
  }
  /* Standard's .std-menu-content/.std-menu-item classes supply background,
     border, shadow, hover/disabled states, and the kbd/shortcut treatment
     (packages/styles/_standard-13-components.scss). Only positioning and
     genuinely Reveal-specific bits (submenu flip, star rating) stay local. */
  .photo-context-menu {
    position: fixed;
    z-index: 9999;
    min-width: 220px;
  }
  .context-submenu {
    position: relative;
  }
  .submenu-arrow {
    font-size: 12px;
    opacity: 0.6;
  }
  .context-submenu-panel {
    display: none;
    position: absolute;
    top: 0;
    left: 100%;
    min-width: 160px;
  }
  .context-submenu-panel.submenu-left {
    left: auto;
    right: 100%;
  }
  .context-submenu:hover .context-submenu-panel {
    display: block;
  }
  .context-rating-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 10px;
  }
  .context-rating-label {
    opacity: 0.6;
    font-size: 11px;
  }
  .context-stars {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .star-btn, .star-clear {
    all: unset;
    cursor: pointer;
    font-size: 13px;
    color: color-mix(in srgb, var(--color-foreground) 30%, transparent);
    padding: 0 2px;
  }
  .star-btn.active {
    color: var(--color-accent);
  }
  .star-btn:hover {
    color: var(--color-accent);
  }
  .star-clear {
    font-size: 10px;
    margin-left: 4px;
    opacity: 0.5;
  }
  .star-clear:hover {
    opacity: 1;
  }
</style>
