<script>
  let {
    photoMenu = null,
    selectedPaths = new Set(),
    installedEditors = [],
    copiedRecipe = null,
    storySet = new Set(),
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
  } = $props();
</script>

{#if photoMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="context-backdrop" onclick={onClose} role="presentation"></div>
  <div
    class="photo-context-menu"
    style="left: {photoMenu.x}px; top: {photoMenu.y}px"
    role="menu"
    tabindex="-1"
    oncontextmenu={(event) => event.preventDefault()}
  >
    <div class="context-header">
      {selectedPaths.size > 1 ? `${selectedPaths.size} photos sélectionnées` : photoMenu.frame.name}
    </div>
    <div class="context-divider"></div>
    <button role="menuitem" onclick={() => { onOpenPhoto(photoMenu.frame.path); onClose(); }}>
      <span>Développer</span>
      <kbd>d</kbd>
    </button>
    <button role="menuitem" onclick={() => { onOpenPreview(photoMenu.frame.path); onClose(); }}>
      <span>Aperçu (Quick Look)</span>
      <kbd>Espace</kbd>
    </button>
    <button role="menuitem" onclick={() => { onRevealInFinder(photoMenu.frame.path); onClose(); }}>
      <span>Afficher dans le Finder</span>
    </button>
    {#if installedEditors.length}
      <div class="context-submenu">
        <button role="menuitem" aria-haspopup="menu">
          <span>Ouvrir avec…</span>
          <span class="submenu-arrow">›</span>
        </button>
        <div
          class="context-submenu-panel"
          class:submenu-left={photoMenu.submenuLeft}
          role="menu"
          tabindex="-1"
        >
          {#each installedEditors as [name, appPath]}
            <button role="menuitem" onclick={() => { onOpenInEditor(photoMenu.frame.path, appPath); onClose(); }}>{name}</button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="context-divider"></div>

    <button role="menuitem" onclick={() => { onCopyImage(photoMenu.frame.path); onClose(); }}>
      <span>Copier l'image</span>
      <kbd>⌘C</kbd>
    </button>
    <button role="menuitem" onclick={() => { onCopySettings(); onClose(); }}>
      <span>Copier les réglages</span>
      <kbd>c</kbd>
    </button>
    <button
      role="menuitem"
      disabled={!copiedRecipe}
      class:disabled={!copiedRecipe}
      onclick={() => { onPasteSettings(); onClose(); }}
    >
      <span>Coller les réglages</span>
      <kbd>v</kbd>
    </button>

    <div class="context-divider"></div>

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

    <button role="menuitem" onclick={() => { onToggleStory(photoMenu.frame.path); onClose(); }}>
      <span>{storySet.has(stem(photoMenu.frame.name)) ? "Retirer de la collection" : "Ajouter à la collection"}</span>
      <kbd>q</kbd>
    </button>

    <div class="context-divider"></div>

    <button role="menuitem" onclick={() => { onExportSelection(); onClose(); }}>
      <span>Exporter la sélection…</span>
      <kbd>r</kbd>
    </button>
    <button role="menuitem" onclick={() => { onDevelopToVault(photoMenu.frame.path); onClose(); }}>
      <span>Envoyer dans la voûte (Obsidian)</span>
    </button>
  </div>
{/if}

<style>
  .context-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9998;
    background: transparent;
  }
  .photo-context-menu {
    position: fixed;
    z-index: 9999;
    min-width: 220px;
    background: rgba(24, 24, 27, 0.95);
    backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    padding: 4px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    color: var(--color-foreground, #f4f4f5);
    font-family: var(--font-text, sans-serif);
    font-size: 12px;
  }
  .context-header {
    padding: 6px 10px;
    font-size: 11px;
    font-family: var(--font-monospace, monospace);
    opacity: 0.6;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .context-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.1);
    margin: 4px 0;
  }
  .photo-context-menu button[role="menuitem"] {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 5px 10px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .photo-context-menu button[role="menuitem"]:hover:not(:disabled) {
    background: var(--color-accent);
    color: #fff;
  }
  .photo-context-menu button[role="menuitem"]:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .photo-context-menu kbd {
    font-family: var(--font-monospace, monospace);
    font-size: 10px;
    opacity: 0.6;
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
    background: rgba(24, 24, 27, 0.95);
    backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    padding: 4px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
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
    color: rgba(255, 255, 255, 0.3);
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
