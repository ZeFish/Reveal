# Sidebar Module (`@modules/sidebar`)

The Reveal floating navigation sidebar: folder tree hierarchy, catalogue roots, editorial filter panel, brand controls, Garden account integration, and folder context modals.

## Architecture

This vertical slice extracts and isolates the monolithic folder browser and catalogue navigation from `+page.svelte` into composable components and pure tree algorithms:

- `treeOperations.js`: Pure data transformations for catalogue trees (`buildCatalogueTrees`), client-side ghosts pruning (`filterGhosts`), and LocalStorage persistence for expanded tree nodes (`readPersistedSet`, `writePersistedSet`).
- `sidebarController.svelte.js`: Unified controller handling sidebar visibility, peek mode on hover, auto-hide timer, and toggle triggers.
- `BrandCluster.svelte`: Window-top controls riding over native traffic lights: sidebar toggle, focus mode trigger, appearance switch (light/dark), and wordmark shortcuts button.
- `SidebarTabs.svelte`: Mode filter switcher (`Frames` ↔ `Editorial`).
- `CatalogueTree.svelte`: Complete directory and catalogue navigation tree with inline rename, new folder creation, drop target highlights, and drag-and-drop mechanics (photos & folders).
- `LibraryNoteDrawer.svelte`: Expandable drawer for viewing and editing global catalogue notes.
- `EditorialSidebar.svelte`: Story / editorial mode body containing theme picker, hero publish action to Garden, local develop and export actions, and live publication status banner.
- `GardenAccountRow.svelte`: Garden authentication and status popover at the base of the sidebar (connect via browser, API key submission, account stats, and disconnect).
- `FolderModals.svelte`: Context menu on folder rows (reveal in Finder, set import folder, reindex, tidy, new folder, rename, remove library) and library deletion confirmation `AlertDialog`.
- `Sidebar.svelte`: Main orchestrator composing these focused components.
- `index.js`: Public barrel exporting components, controllers, and tree operations.

## Test Coverage

- `treeOperations.test.js`: Comprehensive unit tests covering pure catalogue tree derivations, local storage parsing, and ghost folder filtering.
