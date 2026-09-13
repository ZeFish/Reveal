<script>
  import Dialog from "@stnd/ui/Dialog.svelte";
  /**
   * @typedef {Object} Props
   * @property {() => void} [onClose]
   */

  /** @type {Props} */
  let { onClose = () => {} } = $props();
</script>

<Dialog open label="Keyboard shortcuts" onclose={onClose} style="--dialog-width: min(90vw, 760px); --dialog-max-height: 85vh">
    <header class="panel-header">
      <h2>RACCOURCIS CLAVIER</h2>
      <button class="close-btn" onclick={onClose} aria-label="Close keyboard shortcuts">✕</button>
    </header>
    <div class="shortcuts-grid">
      <div class="shortcut-group">
        <h3>NAVIGATION</h3>
        <div class="shortcut-row"><kbd>←</kbd> <kbd>→</kbd> <kbd>↑</kbd> <kbd>↓</kbd> <span>Naviguer la grille</span></div>
        <div class="shortcut-row"><kbd>Espace</kbd> <span>Aperçu — photo seule ↔ grille</span></div>
        <div class="shortcut-row"><kbd>g</kbd> <span>Revenir à la grille</span></div>
        <div class="shortcut-row"><kbd>m</kbd> <span>Mosaïque ↔ grille</span></div>
        <div class="shortcut-row"><kbd>f</kbd> <span>Plein écran</span></div>
        <div class="shortcut-row"><kbd>l</kbd> <span>Basculer clair / sombre (système)</span></div>
        <div class="shortcut-row"><kbd>+</kbd> <span>Agrandir (moins de photos)</span></div>
        <div class="shortcut-row"><kbd>−</kbd> <span>Réduire (plus de photos)</span></div>
        <div class="shortcut-row"><kbd>⇧</kbd> <kbd>+</kbd> / <kbd>⇧</kbd> <kbd>−</kbd> <span>Marge de la grille</span></div>
      </div>
      <div class="shortcut-group">
        <h3>NOTATION & SÉLECTION</h3>
        <div class="shortcut-row"><kbd>1</kbd> … <kbd>5</kbd> <span>Noter la photo</span></div>
        <div class="shortcut-row"><kbd>0</kbd> <span>Retirer la note</span></div>
        <div class="shortcut-row"><kbd>⇧</kbd> clic / flèches <span>Sélection multiple continue</span></div>
        <div class="shortcut-row"><kbd>⌘</kbd> clic / Espace <span>Sélection multiple isolée</span></div>
        <div class="shortcut-row"><kbd>⌘</kbd> <kbd>A</kbd> <span>Tout sélectionner</span></div>
        <div class="shortcut-row"><kbd>⌘</kbd> <kbd>D</kbd> <span>Tout désélectionner</span></div>
        <div class="shortcut-row"><kbd>q</kbd> <span>Ajouter / retirer de la collection rapide</span></div>
      </div>
      <div class="shortcut-group">
        <h3>PANNEAUX</h3>
        <div class="shortcut-row"><kbd>d</kbd> <span>Développer</span></div>
        <div class="shortcut-row"><kbd>⇧</kbd> <kbd>D</kbd> <span>Panneau de développement</span></div>
        <div class="shortcut-row"><kbd>z</kbd> / <kbd>⇧</kbd> <kbd>z</kbd> <span>Zoom : cadre → plein → 100% (⇧ inverse)</span></div>
        <div class="shortcut-row"><kbd>s</kbd> <span>Storytelling</span></div>
        <div class="shortcut-row"><kbd>b</kbd> <span>Panneau des dossiers</span></div>
        <div class="shortcut-row"><kbd>o</kbd> <span>Mode Focus</span></div>
        <div class="shortcut-row"><kbd>Esc</kbd> <span>Revenir à la grille</span></div>
      </div>
      <div class="shortcut-group">
        <h3>RÉGLAGES & ACTION</h3>
        <div class="shortcut-row"><kbd>c</kbd> <span>Copier les réglages</span></div>
        <div class="shortcut-row"><kbd>v</kbd> <span>Coller les réglages (sélection)</span></div>
        <div class="shortcut-row"><kbd>r</kbd> <span>Reveal — développe + exporte la sélection</span></div>
        <div class="shortcut-row"><kbd>?</kbd> / <kbd>h</kbd> <span>Afficher cette aide</span></div>
      </div>
    </div>
</Dialog>

<style>
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
  }
  .panel-header h2 {
    font-family: var(--font-header, sans-serif);
    font-size: 0.9rem;
    letter-spacing: 0.1em;
    margin: 0;
  }
  .close-btn {
    all: unset;
    cursor: pointer;
    opacity: 0.6;
    font-size: 1rem;
    padding: 0.2rem;
  }
  .close-btn:hover {
    opacity: 1;
  }
  .shortcuts-grid {
    padding: 1.25rem;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1.25rem;
  }
  .shortcut-group h3 {
    font-family: var(--font-monospace, monospace);
    font-size: 0.7rem;
    letter-spacing: 0.08em;
    color: var(--color-accent);
    margin: 0 0 0.6rem 0;
  }
  .shortcut-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.78rem;
    margin-bottom: 0.4rem;
  }
  .shortcut-row span {
    margin-left: 0.2rem;
    opacity: 0.8;
  }
  kbd {
    font-family: var(--font-monospace, monospace);
    font-size: 0.7rem;
    padding: 0.15rem 0.4rem;
    background: var(--color-surface-high, #27272a);
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.15));
    border-radius: var(--radius-sm);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
  }
</style>
