# Reveal — Architecture des Engins de Rendu

Ce sous-module implémente l'architecture modulaire et extensible pour les engins de développement numérique et argentique de Reveal.

---

## 1. Principes d'architecture

1. **Indépendance totale des interfaces** : chaque engin possède son propre composant d'interface avec ses réglages, ses dépendances et sa mise en page.
2. **Extensibilité dynamique (Plugin seam)** :
   - Tout engin déclaré côté Rust (`list_engines`) fonctionne **immédiatement** sans écrire de code frontend grâce au moteur de repli schéma [`GenericEngine.svelte`](./GenericEngine.svelte).
   - Un engin peut obtenir une interface sur mesure en s'enregistrant dans [`engineRegistry.js`](./engineRegistry.js).
3. **Composabilité** : les contrôles partagés (curseurs, interrupteurs, menus, mélangeurs de bandes, courbes, empilement de LUTs) sont des primitives réutilisables dans `controls/`.

---

## 2. Structure des dossiers

```
modules/develop/engines/
├── README.md                      # Ce document
├── engineRegistry.js              # Registre dynamique des engins
├── engineRegistry.test.js         # Tests unitaires du registre
├── engineContext.js               # Contexte, ciblage de zones et formatage
├── engineContext.test.js          # Tests unitaires du contexte
├── GenericEngine.svelte           # Moteur générique piloté par le schéma de l'engin
├── controls/                      # Primitives de contrôle réutilisables
│   ├── CollapsibleGroup.svelte   # Section accordéon avec persistance LocalStorage
│   ├── SliderRow.svelte          # Curseur avec double-clic reset et échelle non-linéaire
│   ├── ToggleRow.svelte          # Interrupteur switch
│   ├── SelectRow.svelte          # Menu déroulant de sélection
│   └── BandMixer.svelte          # Sélecteur de bandes chromatiques (Teinte, Saturation, Luma)
├── rapid/                         # Engin GPU numérique
│   └── RapidEngine.svelte        # Interface dédiée : LogC, AgX, courbes, bandes, LUTs
└── spektra/                       # Engin de simulation argentique (spektrafilm-rs)
    └── SpektraEngine.svelte       # Interface dédiée : émulsions, chimie, tirage, flare, DIR
```

---

## 3. Comment ajouter un nouvel engin

### Option A : Déclaration pure Rust (Zéro code frontend requis)
Déclarez le nouvel engin dans `reveal-engine` en implémentant le trait `RenderEngine` et en définissant `control_groups()`. L'engin apparaîtra automatiquement dans Reveal et utilisera [`GenericEngine.svelte`](./GenericEngine.svelte).

### Option B : Interface personnalisée
1. Créez votre dossier : `modules/develop/engines/<mon_engin>/MonEngin.svelte`.
2. Enregistrez-le dans `engineRegistry.js` :
```javascript
import MonEngin from "./mon_engin/MonEngin.svelte";

registerEngine({
  id: "mon_engin",
  label: "Mon Engin",
  component: MonEngin,
  description: "Description de mon engin",
  icon: "aperture",
});
```
3. Votre composant reçoit les props suivantes :
   - `recipe` ($bindable) : la recette de développement active.
   - `activeZone` : `"global"` | `"shadows"` | `"midtones"` | `"highlights"`.
   - `defaults` : les valeurs par défaut issues de Rust (`default_recipe`).
   - `edited(live?: boolean)` : callback pour notifier qu'un réglage a changé.
   - `resetControl(id, index?)` : callback pour réinitialiser un réglage.

---

## 4. Comment retirer un engin

Pour retirer un engin :
- Supprimez son dossier sous `engines/<nom>/`.
- Retirez son appel `registry.set(...)` dans `engineRegistry.js`.
- Rien d'autre dans l'application n'est impacté.
