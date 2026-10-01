<script>
  import ContextMenu from "@stnd/ui/ContextMenu.svelte";
  import MenuItem from "@stnd/ui/ContextMenuItem.svelte";
  import MenuLabel from "@stnd/ui/ContextMenuLabel.svelte";
  import MenuSeparator from "@stnd/ui/ContextMenuSeparator.svelte";
  import Popover from "@stnd/ui/Popover.svelte";
  import Alert from "@stnd/ui/Alert.svelte";
  import AlertDialog from "@stnd/ui/AlertDialog.svelte";
  // The floating sidebar — a 1:1 port of the Swift `sidebarCard` +
  // `folderBrowser` + `FolderTree` (CullView.swift / FolderTree.swift):
  // full-height elevated card, the brand cluster (traffic lights + toggles +
  // wordmark) riding its top, FRAMES/EDITORIAL tabs, "ALL LIBRARY",
  // one section per catalogue, the tree with status dots and story dots,
  // then LIBRARY and the Garden account row at the bottom.
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "$lib/components/Icon.svelte";
  import { storyTheme, setStoryTheme, THEMES } from "$lib/story-theme.svelte.js";
  import { THEME_IDS, DEFAULT_THEME } from "$lib/app-theme.js";
  import { APPLE_PHOTOS_ROOT, photoCollectionAncestors } from "./applePhotosTree.js";

  let {
    root,
    roots = [], // every catalogue root — one tree each; falls back to [root]
    dirs,
    curDir,
    scanning,
    indexProgress = null, // {dirs, frames} while a scan walks the library
    // Grid is the only mode; this is a display filter on top of it — not a
    // destination. `true` swaps the folder tree for the Editorial body
    // (theme + publish actions + notes list), same content, same
    // interactivity.
    previewFilter = false,
    storyDirs = new Set(),
    // Editorial body props — passed by +page.svelte.
    onDevelopStory = () => {},
    onPublishStory = () => {},
    onExportLocalStory = () => {},
    publishing = false,
    // The story is already on the Garden: publishing edits that note.
    storyPublished = false,
    publishStatus = null,
    gardenUrl = null,
    focusOn = false,
    catalogContent = "",
    importDir = null, // chosen import folder (null → no accent); set via context menu
    floating = false,
    onOpenDir,
    onOpenLibrary,
    onRescan,
    onRescanDir,
    /** @type {(path: string) => Promise<void> | void} */
    onRemoveLibrary = () => {},
    onRevealDir,
    onAddLocation,
    /** @type {(path: string) => void} */
    onSetImportDir = () => {},
    onOpenNote,
    onTogglePreview = () => {},
    onToggleSidebar,
    onToggleFocus,
    onToggleAppearance,
    onShowShortcuts,
    onShowSettings,
    onCatalogChange,
    // The Garden account row (Swift `GardenAccountRow`): `garden` is the
    // display state {signed_in, username, tier, notes_count, total_views};
    // sign-in/out return promises so the popover can show verify progress.
    garden = null,
    onGardenSignIn,
    onGardenSignOut,
    onOpenUrl,
    // Drop photos dragged from the grid onto a folder row to move them there.
    onMovePhotos = () => {},
    selectedCount = 0,
    onMoveSelectedPhotos = () => {},
    // File management: rename/move a folder, create a new one. Each returns
    // a Promise so the popover/inline editor can wait before closing.
    onRenameDir = () => {},
    onCreateFolder = () => {},
    onMoveDir = () => {},
    /** @type {import('./applePhotosTree.js').PhotoLibrary | null} */
    applePhotos = null,
    onConnectApplePhotos = () => {},
    onRefreshApplePhotos = () => {},
  } = $props();

  const EXPANDED_KEY = "reveal.sidebar.expanded.v2";
  const MANUALLY_COLLAPSED_KEY = "reveal.sidebar.manuallyCollapsed.v2";

  // Restored synchronously at init — guarded for the prerender pass. The Set
  // is reassigned (never mutated) on toggle, so plain Set reactivity is enough.
  let expanded = $state(readExpanded());
  // A node's full catalogue-qualified path auto-shows when it is an ancestor
  // of the current directory. PhotoKit nodes use their source-prefixed IDs.
  // (see isExpanded) even when it's not in `expanded` — so you can always see
  // where you are. That rule used to unconditionally win, which meant the
  // chevron could never actually collapse an ancestor of whatever folder you
  // were currently viewing (reproduced 2026-08-04: clicking the "2026"
  // chevron while inside 2026/2026-08-01 visibly did nothing). This tracks an
  // explicit "no, collapse it anyway" override that beats the auto-show rule.
  let manuallyCollapsed = $state(readManuallyCollapsed());
  let libOpen = $state(false);
  /** @type {{ path: string, label: string, x: number, y: number } | null} */
  let folderMenu = $state(null);

  let accountOpen = $state(false);
  let pastedKey = $state("");
  let verifying = $state(false);
  /** @type {string | null} */
  let accountError = $state(null);

  const signedIn = $derived(!!garden?.signed_in);

  // The folder's theme is one dropdown: a Garden theme, written to the story
  // note as `theme:` (the key the Garden reads). Colours, fonts and scale come
  // with the theme; there is nothing to tune by hand.
  /** @param {Event & { currentTarget: HTMLSelectElement }} e */
  async function onThemePick(e) {
    const id = e.currentTarget.value || null;
    setStoryTheme(id);
    if (curDir) await invoke("story_set_theme", { dir: curDir, theme: id });
  }

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

  function readExpanded() {
    if (typeof localStorage === "undefined") return new Set();
    try {
      const saved = JSON.parse(localStorage.getItem(EXPANDED_KEY) ?? "null");
      if (Array.isArray(saved)) return new Set(saved);
    } catch (_) {}
    return new Set();
  }

  function readManuallyCollapsed() {
    if (typeof localStorage === "undefined") return new Set();
    try {
      const saved = JSON.parse(localStorage.getItem(MANUALLY_COLLAPSED_KEY) ?? "null");
      if (Array.isArray(saved)) return new Set(saved);
    } catch (_) {}
    return new Set();
  }

  /** @typedef {{ name: string, rel: string, abs: string, count: number, children: TreeNode[], source?: "photos" }} TreeNode */
  const photoAncestors = $derived(new Set(applePhotos?.active
    ? photoCollectionAncestors(applePhotos.albums, applePhotos.album).slice(0, -1)
    : []));

  /** @param {string} rel */
  function isAncestor(rel) {
    return applePhotos?.active ? photoAncestors.has(rel) : !!curDir?.startsWith(rel + "/");
  }

  /** @param {string} rel */
  function toggle(rel) {
    if (isExpanded(rel)) {
      const nextExpanded = new Set(expanded);
      nextExpanded.delete(rel);
      expanded = nextExpanded;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(EXPANDED_KEY, JSON.stringify([...nextExpanded]));
      }
      // The auto-show rule (isExpanded's ancestor check, below) would
      // otherwise immediately re-show this node — remember the explicit
      // collapse so it actually sticks while browsing inside it.
      if (isAncestor(rel)) {
        const nextCollapsed = new Set(manuallyCollapsed);
        nextCollapsed.add(rel);
        manuallyCollapsed = nextCollapsed;
        if (typeof localStorage !== "undefined") {
          localStorage.setItem(MANUALLY_COLLAPSED_KEY, JSON.stringify([...nextCollapsed]));
        }
      }
    } else {
      const nextCollapsed = new Set(manuallyCollapsed);
      nextCollapsed.delete(rel);
      manuallyCollapsed = nextCollapsed;
      const nextExpanded = new Set(expanded);
      nextExpanded.add(rel);
      expanded = nextExpanded;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(MANUALLY_COLLAPSED_KEY, JSON.stringify([...nextCollapsed]));
        localStorage.setItem(EXPANDED_KEY, JSON.stringify([...nextExpanded]));
      }
    }
  }

  const CAT_EXPANDED_KEY = "reveal.sidebar.catalogs.expanded.v2";

  let expandedCatalogs = $state(readExpandedCatalogs());

  function readExpandedCatalogs() {
    if (typeof localStorage === "undefined") return new Set();
    try {
      const saved = JSON.parse(localStorage.getItem(CAT_EXPANDED_KEY) ?? "null");
      if (Array.isArray(saved)) return new Set(saved);
    } catch (_) {}
    return new Set();
  }

  /** @param {string} cat */
  function isCatExpanded(cat) {
    if (cat === APPLE_PHOTOS_ROOT && !applePhotos?.loaded) return false;
    return !expandedCatalogs.has(`collapsed:${cat}`);
  }

  /** @param {string} cat */
  function toggleCatExpanded(cat) {
    const next = new Set(expandedCatalogs);
    const key = `collapsed:${cat}`;
    if (isCatExpanded(cat)) next.add(key);
    else next.delete(key);
    expandedCatalogs = next;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(CAT_EXPANDED_KEY, JSON.stringify([...next]));
    }
  }

  const catalogueTrees = $derived.by(() => {
    // The library is EXACTLY the registered roots — one tree each. We no
    // longer guess catalogues from which folders hold photos directly (that
    // scattered deep libraries like `Personelle/<trip>/Capture/`, where the
    // named folders carry no photos of their own). Intermediate folders are
    // synthesised as nodes by the byRel walk below, so nesting is faithful.
    const rootsList = roots?.length ? [...roots] : root ? [root] : [];
    const catalogues = rootsList.map((catRoot) => {
      let total = 0;
      const isExpanded = isCatExpanded(catRoot);
      /** @type {TreeNode[]} */
      const top = [];

      if (!isExpanded) {
        // Fast path for collapsed catalogues: calculate count without building node maps
        for (const d of dirs) {
          if (d.dir === catRoot || d.dir.startsWith(catRoot + "/")) {
            total += d.count;
          }
        }
      } else {
        /** @type {Map<string, TreeNode>} */
        const byRel = new Map();

        for (const d of dirs) {
          if (d.dir === catRoot || d.dir.startsWith(catRoot + "/")) {
            total += d.count;
          }
          if (d.dir === catRoot || !d.dir.startsWith(catRoot + "/")) continue;
          const rel = d.dir.slice(catRoot.length + 1);
          let acc = "";
          let siblings = top;
          for (const part of rel.split("/")) {
            acc = acc ? `${acc}/${part}` : part;
            let node = byRel.get(acc);
            if (!node) {
              node = { name: part, rel: `${catRoot}/${acc}`, abs: `${catRoot}/${acc}`, count: 0, children: [] };
              byRel.set(acc, node);
              siblings.push(node);
            }
            siblings = node.children;
          }
          const leaf = byRel.get(rel);
          if (leaf) leaf.count = d.count;
        }

        /** @param {TreeNode[]} nodes */
        const sortDesc = (nodes) => {
          nodes.sort((a, b) => b.name.localeCompare(a.name));
          nodes.forEach((n) => sortDesc(n.children));
        };
        sortDesc(top);
      }

      return {
        cat: catRoot,
        name: catRoot.split("/").pop() || catRoot,
        total,
        tree: { nodes: top },
      };
    });
    if (applePhotos?.supported) {
      /** @param {import('./applePhotosTree.js').PhotoCollection[]} collections
       * @returns {TreeNode[]} */
      const photoNodes = (collections) => collections.map((collection) => ({
        name: collection.title,
        rel: APPLE_PHOTOS_ROOT + collection.id,
        abs: APPLE_PHOTOS_ROOT + collection.id,
        count: collection.count ?? 0,
        source: "photos",
        children: photoNodes(collection.children),
      }));
      catalogues.push({
        cat: APPLE_PHOTOS_ROOT, name: "Apple Photos",
        total: applePhotos.total ?? 0,
        tree: { nodes: photoNodes(applePhotos.albums) },
      });
    }
    return catalogues;
  });

  const grandTotal = $derived(dirs.reduce((/** @type {number} */ sum, /** @type {any} */ d) => sum + d.count, 0));

  const isLibrary = $derived(!!root && curDir === root && !applePhotos?.active);

  // A row shows its folder (recursively, subfolders included — the backend
  // query is prefix-based); a row with children also discloses on navigate.
  /** @param {TreeNode} node */
  function navigate(node) {
    if (node.source === "photos" && applePhotos?.busy) return;
    folderMenu = null;
    // Ensure-open, never toggle: `toggle` now flips whatever's currently
    // VISIBLE (including nodes auto-shown because curDir lives inside them),
    // so calling it here on a node that's already showing via that rule
    // would collapse it instead of the no-op this always meant to be.
    if (node.children.length) ensureExpanded(node.rel);
    onOpenDir(node.abs);
  }

  /** Is the right-clicked path a catalogue root rather than a folder in one? */
  const menuPathIsLibrary = $derived.by(() => {
    const path = folderMenu?.path;
    if (!path) return false;
    const known = roots?.length ? roots : root ? [root] : [];
    return known.includes(path);
  });

  /** @type {{path: string, name: string} | null} */
  let libraryToRemove = $state(null);

  /**
   * @param {MouseEvent} event
   * @param {string} path
   * @param {string} label
   */
  function openFolderMenu(event, path, label) {
    event.preventDefault();
    event.stopPropagation();
    folderMenu = {
      path,
      label,
      x: event.clientX,
      y: event.clientY,
    };
  }

  /** @param {(path: string) => void} action */
  function runFolderAction(action) {
    const path = folderMenu?.path;
    folderMenu = null;
    if (path) action(path);
  }

  // Photo drag-and-drop onto a folder row (move). `dropTarget` holds the abs
  // path of the row currently hovered, for the highlight. The dragged frames'
  // paths ride on the drag session under our own MIME type; getData is only
  // readable on drop, so dragover just checks the type is present.
  const PHOTO_MIME = "application/x-reveal-photos";
  /** @type {string | null} */
  let dropTarget = $state(null);

  /** @param {DragEvent} event */
  function canDropPhotos(event) {
    if (!event.dataTransfer) return false;
    const types = Array.from(event.dataTransfer.types || []);
    return types.includes(PHOTO_MIME) || types.includes("text/plain") || types.includes("Files");
  }
  /**
   * @param {DragEvent} event
   * @param {string} abs
   */
  function onRowDragOver(event, abs) {
    if (!canDropPhotos(event) && !canDropFolder(event)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    dropTarget = abs;
  }
  /** @param {string} abs */
  function onRowDragLeave(abs) {
    if (dropTarget === abs) dropTarget = null;
  }
  /**
   * @param {DragEvent} event
   * @param {string} abs
   */
  function onRowDrop(event, abs) {
    if (canDropFolder(event)) {
      onFolderRowDrop(event, abs);
      return;
    }
    if (!canDropPhotos(event)) return;
    event.preventDefault();
    dropTarget = null;
    let paths = [];
    try {
      const raw = event.dataTransfer?.getData(PHOTO_MIME) || event.dataTransfer?.getData("text/plain") || "";
      if (raw.startsWith("[")) {
        paths = JSON.parse(raw);
      } else if (raw) {
        paths = [raw];
      }
    } catch (_) {}
    if (paths.length) onMovePhotos(paths, abs);
  }

  // ---- folder drag-and-drop (move a folder onto another) --------------------
  const FOLDER_MIME = "application/x-reveal-folder";
  /** @type {string | null} */
  let draggingFolder = $state(null); // the abs path currently being dragged

  /** @param {DragEvent} event */
  function canDropFolder(event) {
    return !!event.dataTransfer && [...event.dataTransfer.types].includes(FOLDER_MIME);
  }
  /**
   * @param {DragEvent} event
   * @param {TreeNode} node
   */
  function onFolderDragStart(event, node) {
    event.stopPropagation(); // don't also start a "navigate" click
    draggingFolder = node.abs;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData(FOLDER_MIME, JSON.stringify({ path: node.abs, name: node.name }));
    }
  }
  function onFolderDragEnd() {
    draggingFolder = null;
  }
  /**
   * @param {DragEvent} event
   * @param {string} destAbs
   */
  async function onFolderRowDrop(event, destAbs) {
    event.preventDefault();
    dropTarget = null;
    let payload;
    try {
      payload = JSON.parse(event.dataTransfer?.getData(FOLDER_MIME) ?? "");
    } catch (_) {
      return;
    }
    draggingFolder = null;
    if (!payload?.path || payload.path === destAbs) return;
    // A folder can't drop onto its own descendant — cheap client-side guard;
    // move_dir re-checks server-side against the real filesystem regardless.
    if (destAbs.startsWith(payload.path + "/")) return;
    await onMoveDir(payload.path, destAbs);
  }

  // ---- inline rename ----------------------------------------------------
  /** @type {string | null} */
  let renamingPath = $state(null);
  let renameValue = $state("");

  /**
   * @param {string} path
   * @param {string} currentName
   */
  function beginRename(path, currentName) {
    renamingPath = path;
    renameValue = currentName;
  }
  async function commitRename() {
    const path = renamingPath;
    const value = renameValue.trim();
    renamingPath = null;
    if (!path || !value) return;
    await onRenameDir(path, value);
  }

  // ---- new folder (client-side ghost until it holds real frames) --------
  // An empty folder has zero indexed frames, so it wouldn't otherwise appear
  // in `dirs`-derived `tree` at all. `ghosts` keeps freshly-created empty
  // folders visible (and usable as drop targets) until either something
  // lands in them (they then show up for real and the ghost is pruned) or
  // the sidebar reloads. The folder itself is real on disk either way —
  // this only affects whether it's SHOWN before it has content.
  /** @type {{ abs: string, name: string, parentAbs: string }[]} */
  let ghosts = $state([]); // { abs, name, parentAbs }
  /** @type {string | null} */
  let creatingIn = $state(null); // parent abs path currently showing the "new folder" input
  let createValue = $state("New Folder");

  $effect(() => {
    // Once a ghost's path shows up in the real (index-derived) dirs, drop it.
    const real = new Set(dirs.map((/** @type {any} */ d) => d.dir));
    if (ghosts.some((g) => real.has(g.abs))) {
      ghosts = ghosts.filter((g) => !real.has(g.abs));
    }
  });

  /** @param {string} parentAbs */
  function beginCreateFolder(parentAbs) {
    creatingIn = parentAbs;
    createValue = "New Folder";
    ensureExpanded(parentAbs);
  }
  async function commitCreateFolder() {
    const parentAbs = creatingIn;
    const name = createValue.trim();
    creatingIn = null;
    if (!parentAbs || !name) return;
    try {
      const abs = await onCreateFolder(parentAbs, name);
      if (abs) ghosts = [...ghosts, { abs, name, parentAbs }];
    } catch (_) {
      // onCreateFolder surfaces its own error message upstream.
    }
  }

  /** @param {string} rel */
  function isExpanded(rel) {
    if (manuallyCollapsed.has(rel)) return false;
    if (expanded.has(rel)) return true;
    return isAncestor(rel);
  }
  /** @param {TreeNode} node */
  function isCurrent(node) {
    if (node.source === "photos") return !!applePhotos?.active && node.abs === APPLE_PHOTOS_ROOT + applePhotos.album;
    return !applePhotos?.active && !isLibrary && node.abs === curDir;
  }

  // The import-destination accent: the chosen folder's name turns accent
  // (it used to carry a status dot, a column of circles that cost every row
  // its width for a signal only an import needs). Both are derived client-side from the
  // folder's absolute path — no new data from the backend.
  /** @param {string} abs */
  function isImportDest(abs) {
    return !!importDir && importDir === abs;
  }
  /** @param {string} abs */
  function isImportBranch(abs) {
    return !!importDir && (importDir === abs || importDir.startsWith(abs + "/"));
  }

  /** @param {string|null} rel */
  function ensureExpanded(rel) {
    if (!rel) return;
    if (manuallyCollapsed.has(rel)) {
      const nextCollapsed = new Set(manuallyCollapsed);
      nextCollapsed.delete(rel);
      manuallyCollapsed = nextCollapsed;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(MANUALLY_COLLAPSED_KEY, JSON.stringify([...nextCollapsed]));
      }
    }
    if (expanded.has(rel)) return;
    const next = new Set(expanded);
    next.add(rel);
    expanded = next;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(EXPANDED_KEY, JSON.stringify([...next]));
    }
  }
  /** @param {string} parentAbs */
  function ghostsUnder(parentAbs) {
    return ghosts.filter((g) => g.parentAbs === parentAbs);
  }
  /** @param {TreeNode} node */
  function hasChildren(node) {
    return node.children.length > 0 || ghostsUnder(node.abs).length > 0;
  }

  /** @param {HTMLInputElement} node */
  const selectOnFocus = (node) => {
    node.focus();
    node.select();
  };


</script>

{#snippet row(/** @type {TreeNode} */ node, /** @type {number} */ depth)}
  <div
    class="item dir-row"
    class:import-dest={isImportDest(node.abs)}
    class:import-branch={isImportBranch(node.abs)}
    class:is-drop-target={dropTarget === node.abs}
    class:is-dragging={draggingFolder === node.abs}
    style="--depth: {depth}"
    role="button"
    tabindex="0"
    draggable={node.source !== "photos"}
    aria-disabled={node.source === "photos" && applePhotos?.busy}
    aria-label={node.name}
    aria-current={isCurrent(node) ? "true" : undefined}
    onclick={() => navigate(node)}
    oncontextmenu={(event) => node.source === "photos" ? event.preventDefault() : openFolderMenu(event, node.abs, node.name)}
    ondragstart={node.source === "photos" ? undefined : (event) => onFolderDragStart(event, node)}
    ondragend={node.source === "photos" ? undefined : onFolderDragEnd}
    ondragover={node.source === "photos" ? undefined : (event) => onRowDragOver(event, node.abs)}
    ondragleave={node.source === "photos" ? undefined : () => onRowDragLeave(node.abs)}
    ondrop={node.source === "photos" ? undefined : (event) => onRowDrop(event, node.abs)}
    onkeydown={(e) => {
      if (e.target !== e.currentTarget) return;
      // Space on a row that merely kept focus after a mouse click belongs to
      // the photos (quick look); it is only the row's own when the row was
      // reached by keyboard.
      if (e.key === "Enter" || (e.key === " " && e.currentTarget.matches(":focus-visible"))) {
        e.preventDefault();
        e.stopPropagation();
        navigate(node);
      }
    }}
  >
    {#if hasChildren(node)}
      <button
        class="disc"
        class:open={isExpanded(node.rel)}
        onclick={(e) => {
          e.stopPropagation();
          toggle(node.rel);
        }}
        aria-label={`${isExpanded(node.rel) ? "Collapse" : "Expand"} ${node.name}`}
        aria-expanded={isExpanded(node.rel)}
      >
        <Icon name="caret-right" size="9px" />
      </button>
    {:else}
      <span class="disc"></span>
    {/if}
    {#if renamingPath === node.abs}
      <input
        class="dir-rename"
        value={renameValue}
        oninput={(e) => (renameValue = e.currentTarget.value)}
        onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") commitRename();
          else if (e.key === "Escape") renamingPath = null;
        }}
        onblur={commitRename}
        onclick={(e) => e.stopPropagation()}
        use:selectOnFocus
      />
    {:else}
      <span class="dir-name" title={node.abs}>{node.name}</span>
    {/if}
    <span class="dir-spacer"></span>
    <span class="dir-count">{node.count > 0 ? node.count : ""}</span>
    <!-- Always reserved, story or not, so the count's right edge is constant
         down the whole list (the story dot no longer shoves the number left). -->
    <span class="story-slot">
      {#if storyDirs.has(node.abs)}
        <span class="story-dot" title="This folder contains a story"></span>
      {/if}
    </span>
  </div>
  {#if hasChildren(node) && isExpanded(node.rel)}
    {#each node.children as child (child.rel)}
      {@render row(child, depth + 1)}
    {/each}
    {#each ghostsUnder(node.abs) as g (g.abs)}
      {@render ghostRow(g, depth + 1)}
    {/each}
  {/if}
  {#if creatingIn === node.abs}
    {@render newFolderInput(depth + 1)}
  {/if}
{/snippet}

{#snippet ghostRow(/** @type {{ abs: string, name: string, parentAbs: string }} */ g, /** @type {number} */ depth)}
  <div
    class="item dir-row"
    aria-current={curDir === g.abs ? "true" : undefined}
    class:is-drop-target={dropTarget === g.abs}
    style="--depth: {depth}"
    role="button"
    tabindex="0"
    onclick={() => onOpenDir(g.abs)}
    ondragover={(event) => onRowDragOver(event, g.abs)}
    ondragleave={() => onRowDragLeave(g.abs)}
    ondrop={(event) => onRowDrop(event, g.abs)}
    onkeydown={(e) => {
      if (e.key === "Enter") onOpenDir(g.abs);
    }}
  >
    <span class="disc"></span>
    <span class="dir-name" title={g.abs}>{g.name}</span>
    <span class="dir-spacer"></span>
    <span class="dir-count"></span>
    <span class="story-slot"></span>
  </div>
{/snippet}

{#snippet newFolderInput(/** @type {number} */ depth)}
  <div class="dir-row" style="--depth: {depth}">
    <span class="disc"></span>
    <input
      class="dir-rename"
      value={createValue}
      oninput={(e) => (createValue = e.currentTarget.value)}
      onkeydown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") commitCreateFolder();
        else if (e.key === "Escape") creatingIn = null;
      }}
      onblur={commitCreateFolder}
      onclick={(e) => e.stopPropagation()}
      use:selectOnFocus
    />
    <span class="dir-spacer"></span>
  </div>
{/snippet}

<nav class="pane" class:floating>
  <!-- Brand cluster — rides the top of the card; native traffic lights
       overlay the window top-left, so the row starts past them. -->
  <div class="brand" data-tauri-drag-region>
    <button class="ghost icon small" onclick={onToggleSidebar} title="Folder panel (B)">
      <Icon name="sidebar-simple" size="12px" />
    </button>
    <button
      class="ghost icon small"
      aria-pressed={focusOn}
      onclick={onToggleFocus}
      title="Focus mode (O)"
    >
      <span class="focus-glyph" class:on={focusOn}></span>
    </button>
    <button class="ghost icon small" onclick={onToggleAppearance} title="Toggle system light/dark appearance (L)">
      <Icon name="circle-half" size="12px" />
    </button>
    <button class="wordmark" onclick={onShowShortcuts} title="Keyboard shortcuts">REVEAL</button>
  </div>

  <!-- FRAMES ↔ EDITORIAL — a display filter on the same grid, not a mode.
       Both tabs call the same toggle; only Editorial is guarded (nothing to
       compose without a folder or in the read-only Apple Photos library). -->
  <div class="tabs btn-group" role="group" aria-label="View">
    <button aria-pressed={!previewFilter} onclick={() => previewFilter && onTogglePreview()}>Frames</button>
    <button
      aria-pressed={previewFilter}
      disabled={isLibrary || applePhotos?.active}
      onclick={() => !previewFilter && onTogglePreview()}
      title={isLibrary ? "Choose a folder to compose a story" : "Editorial (S)"}
    >Editorial</button>
  </div>

  {#if !previewFilter}
    <div class="tree">
      <!-- The index-wide view — the base of everything. -->
      <div
        class="item lib-row"
      aria-current={isLibrary ? "true" : undefined}
      class:is-drop-target={root && dropTarget === root}
      role="button"
      tabindex="0"
      aria-disabled={!root}
      onclick={() => { if (root) onOpenLibrary(); }}
      oncontextmenu={(event) => root && openFolderMenu(event, root, "Library")}
      ondragover={(event) => root && onRowDragOver(event, root)}
      ondragleave={() => root && onRowDragLeave(root)}
      ondrop={(event) => root && onRowDrop(event, root)}
      onkeydown={(e) => {
        if (e.key === "Enter" && root && e.target === e.currentTarget) onOpenLibrary();
      }}
    >
      <span class="lead" aria-hidden="true"></span>
      <span class="lib-label">All Library</span>
      <span class="dir-spacer"></span>
      {#if scanning}
        <!-- The indexer's live count rides here — never over the photos. -->
        <span class="indexing" title="Indexing">
          <Icon name="arrows-clockwise" size="9px" class="spin" />
          <span class="idx-count">{(indexProgress?.frames ?? grandTotal).toLocaleString("en-CA")}</span>
        </span>
      {:else}
        <span class="dir-count">{grandTotal.toLocaleString("en-CA")}</span>
      {/if}
      <button
        class="add-btn"
        onclick={(e) => {
          e.stopPropagation();
          onAddLocation();
        }}
        title="Add catalogue"
      >
        <Icon name="plus" size="9px" />
      </button>
    </div>

    <!-- Section per catalogue with collapse toggle & open handling -->
    {#each catalogueTrees as { cat, name, total, tree } (cat)}
      {@const open = isCatExpanded(cat)}
      {@const photos = cat === APPLE_PHOTOS_ROOT}
      <div class="section">
        <button
          class="cat-disc"
          onclick={(e) => {
            e.stopPropagation();
            toggleCatExpanded(cat);
            if (photos && !open && !applePhotos?.loaded) onConnectApplePhotos();
          }}
          disabled={photos && applePhotos?.busy && !applePhotos.loaded}
          aria-label={`${open ? "Collapse" : "Expand"} ${name}`}
          aria-expanded={open}
          title={open ? "Collapse catalogue" : "Expand catalogue"}
        >
          <span class="disc" class:open><Icon name="caret-right" size="9px" /></span>
        </button>
        <button
          class="section-main ghost"
          class:import-dest={isImportDest(cat)}
          class:import-branch={isImportBranch(cat)}
          title={photos ? "Apple Photos" : cat}
          aria-current={(photos ? applePhotos?.active && !applePhotos.album : curDir === cat) ? "true" : undefined}
          disabled={photos && applePhotos?.busy}
          onclick={() => {
            if (!isCatExpanded(cat)) toggleCatExpanded(cat);
            onOpenDir(cat);
          }}
          oncontextmenu={(event) => photos ? event.preventDefault() : openFolderMenu(event, cat, name)}
        >
          <span class="section-name">{name}</span>
        </button>
        <span class="dir-spacer"></span>
        <span class="dir-count">{!photos || applePhotos?.loaded ? total.toLocaleString("en-CA") : ""}</span>
        <button
          class="add-btn"
          onclick={(e) => {
            e.stopPropagation();
            if (photos) onRefreshApplePhotos();
            else onRescanDir(cat);
          }}
          disabled={photos ? applePhotos?.busy : scanning}
          title={photos ? "Refresh Apple Photos" : "Reindex this catalogue"}
          aria-label={photos ? "Refresh Apple Photos" : `Reindex ${name}`}
          aria-busy={photos ? applePhotos?.busy : scanning}
        >
          <Icon name="arrows-clockwise" size="9px" class={(photos ? applePhotos?.busy : scanning) ? "spin" : ""} />
        </button>
      </div>

      {#if open}
        {#each tree.nodes as node (node.rel)}
          {@render row(node, 0)}
        {/each}
        {#each ghostsUnder(cat) as g (g.abs)}
          {@render ghostRow(g, 0)}
        {/each}
        {#if creatingIn === cat}
          {@render newFolderInput(0)}
        {/if}
      {/if}
    {/each}
  </div>

  <!-- The library's live state — below the folder tree, out of the way. -->
  <div class="lib-section">
    <button class="lib-toggle" onclick={() => (libOpen = !libOpen)}>
      <span>Library</span>
      <span class="disc" class:open={libOpen}><Icon name="caret-right" size="9px" /></span>
    </button>
    {#if libOpen}
      <textarea
        class="lib-note"
        rows="3"
        placeholder="Catalogue notes…"
        value={catalogContent}
        oninput={(e) => onCatalogChange(e.currentTarget.value)}
      ></textarea>
      <button class="lib-open-note" onclick={onOpenNote}>
        <Icon name="note-pencil" size="10px" />
        <span>Open note</span>
      </button>
    {/if}
    </div>
  {:else}
    <!-- Editorial filter body: THEME + PINNED + RECENT — port of Swift
         `folderBrowser` STORY branch (CullView.swift:531-578). Brand cluster
         + tabs unchanged; this only ever swaps in over the SAME grid. -->
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
            <!-- Publish only makes sense signed into a Garden account —
                 without that the button just leads to a network error.
                 Develop and Export stay local, so they're always available:
                 Reveal has to stay usable without ever connecting to the
                 garden. -->
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
              <button class="action-btn secondary" onclick={() => onDevelopStory()} title="Develop every photo in the story">
                <Icon name="sliders-horizontal" size="10px" />
                <span>Develop</span>
              </button>
              <button class="action-btn secondary" onclick={() => onExportLocalStory()} disabled={publishing} title="Export the photos locally">
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
  {/if}

  <div class="divider"></div>

  <!-- The Garden account — bottom-most, a first-class citizen (Swift
       `GardenAccountRow`): signed out, a quiet invite; signed in, the
       initial-avatar + username, with popovers riding above the row. -->
  <div class="account">
    <Popover bind:open={accountOpen} label="Garden account" side="top" onclose={() => { accountError = null; }}>
      {#snippet trigger(/** @type {import('svelte/elements').HTMLButtonAttributes} */ attributes)}
        <button class="account-id" {...attributes}>
          {#if signedIn}
            <span class="account-avatar">{(garden?.username ?? "?").slice(0, 1).toUpperCase()}</span>
            <span class="account-name">{garden?.username}</span>
          {:else}
            <Icon name="user-circle" size="14px" />
            <span>Connect Garden</span>
          {/if}
        </button>
      {/snippet}
      <div class="account-form">
      {#if !signedIn}
        <span class="pop-title">Garden account</span>
        <button class="pop-primary" onclick={() => onOpenUrl?.("https://standard.garden/connect/reveal")}>
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
      {:else}
        <span class="pop-title">Garden account</span>
        <span class="pop-user">{garden?.username}</span>
        {#if garden?.tier}
          <span class="pop-meta">{garden.tier.toUpperCase()}</span>
        {/if}
        <span class="pop-meta">{garden?.notes_count ?? 0} notes · {garden?.total_views ?? 0} views</span>
        <div class="pop-divider"></div>
        <button class="pop-danger" onclick={signOut}>Disconnect</button>
      {/if}
      </div>
    </Popover>
    <span class="dir-spacer"></span>
    <button class="ghost icon small" onclick={onShowSettings} title="Reveal settings">
      <Icon name="gear" size="12px" />
    </button>
  </div>
</nav>

<AlertDialog
  open={!!libraryToRemove}
  title={libraryToRemove ? `Remove “${libraryToRemove.name}” from Reveal?` : ""}
  description="No photo is deleted — the files stay exactly where they are on disk. Reveal forgets this library, along with the ratings, captions and story marks its catalogue holds for them. Adding the folder back and reindexing restores the photos, not those marks."
  confirmLabel="Remove library" cancelLabel="Keep it" intent="danger"
  onconfirm={() => { const l = libraryToRemove; libraryToRemove = null; if (l) onRemoveLibrary(l.path); }}
  oncancel={() => { libraryToRemove = null; }} />

{#if folderMenu}
  <ContextMenu open position={folderMenu} label={`${folderMenu.label} actions`} onclose={() => { folderMenu = null; }}>
  {#snippet content()}
    <MenuLabel label={folderMenu?.label} />
    <MenuSeparator />

    {#if selectedCount > 0}
      <MenuItem
        onclick={() => {
          const path = folderMenu?.path;
          folderMenu = null;
          if (path) onMoveSelectedPhotos(path);
        }}
      >
        Move {selectedCount} photo{selectedCount > 1 ? 's' : ''} here
      </MenuItem>
      <MenuSeparator />
    {/if}

    <MenuItem label="Show in Finder" onclick={() => runFolderAction(onRevealDir)} />
    <MenuItem
      onclick={() => runFolderAction(/** @type {(path: string) => void} */ (onSetImportDir))}
      title="Future imports will use this folder"
    >
      {folderMenu?.path === importDir ? "✓ Active import folder" : "Set as import folder"}
    </MenuItem>
    <MenuItem label="Reindex folder" onclick={() => runFolderAction(onRescanDir)} disabled={scanning} />

    <MenuSeparator />

    <MenuItem
        onclick={() => {
          const path = folderMenu?.path;
          folderMenu = null;
          if (path) beginCreateFolder(path);
        }}
    >
      New folder…
    </MenuItem>

    {#if folderMenu?.path !== root && !menuPathIsLibrary}
      <MenuItem
        onclick={() => {
          const path = folderMenu?.path;
          const name = folderMenu?.label;
          folderMenu = null;
          if (path && name) beginRename(path, name);
        }}
      >
        Rename…
      </MenuItem>
    {/if}

    <!-- A library root is not an ordinary folder: renaming it would break the
         catalogue's own reference to it, and removing it is the one action
         that only exists here. -->
    {#if menuPathIsLibrary}
      <MenuSeparator />
      <MenuItem
        onclick={() => {
          const path = folderMenu?.path;
          const name = folderMenu?.label;
          folderMenu = null;
          if (path) libraryToRemove = { path, name: name || path };
        }}
      >
        Remove library…
      </MenuItem>
    {/if}
  {/snippet}
  </ContextMenu>
{/if}

<style>
  /* The one elevated surface — rounded, hairline, soft shadow, floating
     over the recessed grain base with a uniform 8px inset, full height. */
  nav {
    width: 224px;
    flex-shrink: 0;
    /* A pane floating in the window: inset, concentric corner, raised.
       --shadow-raised already draws the hairline ring, so the border is
       the theme's own --border (zero width in many themes), not a second
       hardcoded 1px on top of it. */
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  nav.floating {
    height: min(480px, calc(100vh - 60px));
    margin: 0;
  }
  nav.floating .brand {
    display: none;
  }

  .brand {
    /* The title-bar band, measured from the WINDOW's top edge: the negative
       margin cancels the card's own inset, so these controls centre on the
       traffic lights and on the rail beside them. */
    height: var(--titlebar-height);
    margin-top: calc(var(--window-inset) * -1.5);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    /* Native traffic lights overlay the window top-left — start past them. */
    padding: 0 var(--space) 0 var(--window-controls-offset-sidebar, 78px);
  }
  /* circle.circle — a ring with a centred dot, accent when focus is on. */
  .focus-glyph {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: var(--stroke-width) solid currentColor;
    position: relative;
  }
  .focus-glyph::after {
    content: "";
    position: absolute;
    inset: 2px;
    border-radius: 50%;
    border: var(--stroke-width) solid currentColor;
  }
  .focus-glyph.on::after {
    background: currentColor;
  }
  /* A 1em line box, NOT the cap trim the rest of the title bar uses: the
     theme's header font declares a cap height larger than its drawn caps,
     so trimming to it put the wordmark 1.5pt high (measured 2026-09-23,
     19.25 against the lights' 20.75). */
  .wordmark {
    cursor: pointer;
  }

  /* The Frames / Editorial switch is the framework's .btn-group; this only
     places it in the card. */
  .tabs {
    flex-shrink: 0;
  }

  .tree {
    flex: 1;
    overflow-y: auto;
    padding-block: var(--space-d2);
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .lib-row,
  .dir-row {
    display: flex;
    align-items: center;
  }
  /* Every row in the tree — All Library, a catalogue, a folder — shares the
     same columns: a caret slot (--lead), the name, the count, and a trailing
     icon slot (--trail). Same gap, same horizontal padding, so names start on
     one line and counts and icons end on another. A child folder steps in by
     6px — less than a caret, so the name column survives deep trees; the
     caret column still reads the hierarchy. */
  .tree {
    --lead: 10px;
    --trail: 14px;
    --row-gap: 5px;
    --indent: 6px;
  }
  .lib-row {
    gap: var(--row-gap);
    padding: var(--space-d5) 0;
  }
  .lead {
    width: var(--lead);
    flex-shrink: 0;
  }
  .lib-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .indexing {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d5);
    color: var(--color-accent);
  }
  .idx-count {
    font-variant-numeric: tabular-nums;
  }
  .indexing :global(.icon),
  .tree :global(.icon.spin) {
    animation: spin 1.2s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .add-btn {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--trail);
    height: 14px;
    flex-shrink: 0;
  }
  .add-btn:disabled {
    cursor: default;
    opacity: 0.4;
  }

  .section {
    display: flex;
    align-items: center;
    gap: var(--row-gap);
    padding: var(--space-d5) 0;
    margin-top: 0;
  }
  .cat-disc {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--lead);
    height: 14px;
    flex-shrink: 0;
    padding:0;
  }
  .section-main {
    text-box: cap alphabetic;
    min-width: 0;
    cursor: pointer;
    padding:0;
  }
  .section-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .section-main.import-dest .section-name,
  .dir-row.import-dest .dir-name {
    color: var(--color-accent);
  }

  .dir-row {
    gap: var(--row-gap);
    /* Tight: the name's own line box already gives a row its height, and with
       6px above and below plus a gap between rows the tree read as a list of
       separate buttons rather than one outline. */
    padding: var(--space-d5) 0 var(--space-d5) calc((var(--depth) + 1) * var(--indent));
    position: relative;
    transition: all var(--transition-fast);
  }
  /* A photo (or another folder) is being dragged over this folder — it will
     land here on drop. */
  /* This folder is the one currently being dragged — it stays put (folders
     aren't reordered by dragging, only moved into another), just quieted so
     the row you're dragging FROM doesn't visually compete with the target. */
  /* Inline rename / new-folder editor — same footprint as .dir-name so the
     row doesn't jump when it switches between text and input. */
  .dir-rename {
    flex: 1;
    min-width: 0;
    padding: var(--stroke-width) var(--space-d4);
  }
  .disc {
    all: unset;
    cursor: pointer;
    width: var(--lead);
    height: 8px;
    box-sizing: content-box;
    /* The visible chevron stays 8px, but an 8x8 hit target is easy to miss
       by a couple pixels — a near-miss lands on .dir-row instead, which
       only ever EXPANDS (never collapses) and no-ops when the folder is
       already current, so the click appeared to do nothing (reproduced
       2026-08-04). Padding widens the clickable area to 20x20 without
       shifting layout — the matching negative margin cancels the padding's
       footprint, so siblings sit exactly where they did before. */
    padding: var(--space-d3);
    margin: calc(var(--space-d3) * -1);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: var(--color-muted);
  }
  .disc:hover,
  .disc.open {
    color: var(--color-foreground);
  }
  .disc :global(.icon) {
    transition: all var(--transition-fast);
  }
  .disc.open :global(.icon) {
    transform: rotate(90deg);
  }
  span.disc {
    cursor: default;
  }

  .dir-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
    transition: color var(--duration-instant);
  }
  .dir-spacer {
    flex: 1;
  }
  .dir-count {
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
    /* Right-aligned in a fixed slot so every count shares one right edge. */
    min-width: 2.4em;
    text-align: right;
  }
  /* Fixed slot for the story marker — present on every row (empty or not) so it
     never shifts the count. */
  .story-slot {
    width: var(--trail);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  /* The Leica whisper — this folder carries a story. */
  .story-dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--color-accent);
    flex-shrink: 0;
  }

  .lib-section {
    flex-shrink: 0;
    padding-block: var(--space-d2);
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }
  .lib-toggle {
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .lib-note {
    padding: var(--space-d3);
    resize: vertical;
  }
  .lib-open-note {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--space-d4);
  }

  .divider {
    height: 1px;
    background: var(--color-border);
    flex-shrink: 0;
  }

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
  /* The initial-in-a-circle avatar — accent-tinted, like Swift's. */
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

  .account-form {
    width: 196px;
    display: flex;
    flex-direction: column;
    gap: var(--space-d2);
    padding: calc(var(--space-d4) * 3);
  }
  .pop-or {
    text-align: center;
  }
  .pop-key {
    padding: var(--space-d3);
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
    cursor: pointer;
  }

  /* Editorial filter body (THEME + PINNED + RECENT) — replaces the folder
     tree + Library when previewFilter is on. */
  .story-body {
    display: flex;
    flex-direction: column;
    gap: var(--space);
    padding: 0 var(--space);
    flex: 1;
    overflow-y: auto;
  }
  /* Pushes Publish/Develop/Export to the bottom of the panel when Pinned +
     Recent don't fill it; collapses to 0 and just falls in reading order
     right after them once the list is long enough to scroll. */
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
  .action-btn.secondary {
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
