<script>
  import ContextMenu from "@stnd/ui/ContextMenu.svelte";
  import MenuItem from "@stnd/ui/ContextMenuItem.svelte";
  import MenuLabel from "@stnd/ui/ContextMenuLabel.svelte";
  import MenuSeparator from "@stnd/ui/ContextMenuSeparator.svelte";
  import Popover from "@stnd/ui/Popover.svelte";
  import Alert from "@stnd/ui/Alert.svelte";
  // The floating sidebar — a 1:1 port of the Swift `sidebarCard` +
  // `folderBrowser` + `FolderTree` (CullView.swift / FolderTree.swift):
  // full-height elevated card, the brand cluster (traffic lights + toggles +
  // wordmark) riding its top, FRAMES/EDITORIAL tabs, "ALL LIBRARY",
  // one section per catalogue, the tree with status dots and story dots,
  // then LIBRARY and the Garden account row at the bottom.
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "$lib/components/Icon.svelte";
  import StoryNotesList from "./StoryNotesList.svelte";
  import gardenThemes from "$lib/garden-themes.generated.json";
  import { storyTheme, updateStoryTheme, DEFAULT_DARK_BG, DEFAULT_ACCENT } from "$lib/story-theme.svelte.js";
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
    // Editorial body props — passed by +page.svelte. onSetPinned is
    // (notePath, pinned), onReorderPinned is (fromIndex, toIndex) — both
    // return promises upstream.
    pinnedStories = [],
    recentStories = [],
    onDevelopStory = () => {},
    onPublishStory = () => {},
    onExportLocalStory = () => {},
    onSetPinned = () => {},
    onReorderPinned = () => {},
    publishing = false,
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

  // Folder theme — a plain dropdown (the richer specimen-card/curated-chips
  // picker, StoryThemePanel.svelte, was tried and dropped — Francis: "the
  // theme selector I hate it. Replace with a simple dropdown."), plus a
  // fold-out for the fine-tune override Francis asked back afterward:
  // "the panel should offer the ability to customize the style tokens like
  // fonts and color." Both read/write the same storyTheme reactive
  // singleton +page.svelte's own curDir effect populates, so this and the
  // app-wide chrome never disagree about which theme is active.
  const THEMES = gardenThemes.themes;
  const currentThemeId = $derived.by(() => {
    const match = THEMES.find(
      (t) =>
        t.darkBackground?.toLowerCase() === storyTheme.darkBackground?.toLowerCase() &&
        t.darkAccent?.toLowerCase() === storyTheme.darkAccent?.toLowerCase()
    );
    return match ? String(match.id) : "";
  });
  let customizeOpen = $state(false);
  let previewBg = $derived(storyTheme.darkBackground ?? DEFAULT_DARK_BG);
  let previewAccent = $derived(storyTheme.darkAccent ?? DEFAULT_ACCENT);

  const FONTS = [
    { label: "System", value: null },
    { label: "Söhne", value: "Sohne" },
    { label: "Avenir Next", value: "Avenir Next" },
    { label: "Lexend", value: "Lexend" },
    { label: "Instrument Sans", value: "Instrument Sans" },
    { label: "Baskerville", value: "Baskerville" },
    { label: "Bookerly", value: "Bookerly" },
    { label: "Adobe Jenson Pro", value: "Adobe Jenson Pro" },
    { label: "New Burns", value: "New Burns" },
    { label: "National Park", value: "National Park" },
    { label: "Wonder", value: "Wonder" },
  ];

  /** @type {ReturnType<typeof setTimeout> | null} */
  let themeSaveTimer = null;
  /** @param {{darkBackground?: string|null, darkAccent?: string|null, fontHeader?: string|null, fontText?: string|null}} partial */
  function applyTheme(partial) {
    updateStoryTheme(partial);
    if (!curDir) return;
    if (themeSaveTimer) clearTimeout(themeSaveTimer);
    themeSaveTimer = setTimeout(async () => {
      const dark_bg = storyTheme.darkBackground;
      await invoke("story_set_theme", {
        dir: curDir,
        tokens: {
          darkBackground: dark_bg,
          darkForeground: null,
          lightBackground: null,
          lightForeground: null,
          darkAccent: storyTheme.darkAccent,
          lightAccent: null,
          fontHeader: storyTheme.fontHeader,
          fontText: storyTheme.fontText,
        },
      });
    }, 250);
  }

  /** @param {Event & { currentTarget: HTMLSelectElement }} e */
  function onThemePick(e) {
    const id = e.currentTarget.value;
    const t = id ? THEMES.find((theme) => String(theme.id) === id) : null;
    applyTheme({
      darkBackground: t?.darkBackground ?? null,
      darkAccent: t?.darkAccent ?? null,
      fontHeader: t?.fontHeader ?? null,
      fontText: t?.fontText ?? null,
    });
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

  // The import-destination accent: the chosen folder gets a filled accent
  // dot, every ancestor gets a faint left-edge stripe so you can trace the
  // branch down to the destination. Both are derived client-side from the
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

  // Editorial pin toggles — typed thin wrappers over the defaulted callback
  // props so the inline arrow params aren't implicit `any` under strict mode.
  /** @param {string} notePath */
  const unpinStory = (notePath) => onSetPinned(notePath, false);
  /** @param {string} notePath */
  const pinStory = (notePath) => onSetPinned(notePath, true);

</script>

{#snippet statusDot(/** @type {boolean} */ filled, /** @type {boolean} */ accent)}
  <span class="dot" class:filled class:accent aria-hidden="true"></span>
{/snippet}

{#snippet row(/** @type {TreeNode} */ node, /** @type {number} */ depth)}
  <div
    class="dir-row"
    class:current={isCurrent(node)}
    class:import-dest={isImportDest(node.abs)}
    class:import-branch={isImportBranch(node.abs)}
    class:drop-target={dropTarget === node.abs}
    class:dragging={draggingFolder === node.abs}
    style="--depth: {depth}"
    role="button"
    tabindex="0"
    draggable={node.source !== "photos"}
    aria-disabled={node.source === "photos" && applePhotos?.busy}
    aria-label={node.name}
    aria-pressed={isCurrent(node)}
    onclick={() => navigate(node)}
    oncontextmenu={(event) => node.source === "photos" ? event.preventDefault() : openFolderMenu(event, node.abs, node.name)}
    ondragstart={node.source === "photos" ? undefined : (event) => onFolderDragStart(event, node)}
    ondragend={node.source === "photos" ? undefined : onFolderDragEnd}
    ondragover={node.source === "photos" ? undefined : (event) => onRowDragOver(event, node.abs)}
    ondragleave={node.source === "photos" ? undefined : () => onRowDragLeave(node.abs)}
    ondrop={node.source === "photos" ? undefined : (event) => onRowDrop(event, node.abs)}
    onkeydown={(e) => {
      if (e.target !== e.currentTarget) return;
      if (e.key === "Enter" || e.key === " ") {
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
    {@render statusDot(isCurrent(node), isImportDest(node.abs))}
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
    class="dir-row"
    class:current={curDir === g.abs}
    class:drop-target={dropTarget === g.abs}
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
    {@render statusDot(curDir === g.abs, false)}
    <span class="dir-name" title={g.abs}>{g.name}</span>
    <span class="dir-spacer"></span>
    <span class="dir-count"></span>
    <span class="story-slot"></span>
  </div>
{/snippet}

{#snippet newFolderInput(/** @type {number} */ depth)}
  <div class="dir-row" style="--depth: {depth}">
    <span class="disc"></span>
    {@render statusDot(false, false)}
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

<nav class:floating>
  <!-- Brand cluster — rides the top of the card; native traffic lights
       overlay the window top-left, so the row starts past them. -->
  <div class="brand" data-tauri-drag-region>
    <button class="chrome-btn" onclick={onToggleSidebar} title="Folder panel (B)">
      <Icon name="sidebar-simple" size="12px" />
    </button>
    <button
      class="chrome-btn"
      class:on={focusOn}
      onclick={onToggleFocus}
      title="Focus mode (O)"
    >
      <span class="focus-glyph" class:on={focusOn}></span>
    </button>
    <button class="chrome-btn" onclick={onToggleAppearance} title="Toggle system light/dark appearance (L)">
      <Icon name="circle-half" size="12px" />
    </button>
    <button class="wordmark" onclick={onShowShortcuts} title="Keyboard shortcuts">REVEAL</button>
  </div>

  <!-- FRAMES ↔ EDITORIAL — a display filter on the same grid, not a mode.
       Both tabs call the same toggle; only Editorial is guarded (nothing to
       compose without a folder or in the read-only Apple Photos library). -->
  <div class="tabs">
    <button class="tab" class:active={!previewFilter} onclick={() => previewFilter && onTogglePreview()}>Frames</button>
    <button
      class="tab"
      class:active={previewFilter}
      disabled={isLibrary || applePhotos?.active}
      onclick={() => !previewFilter && onTogglePreview()}
      title={isLibrary ? "Choose a folder to compose a story" : "Editorial (S)"}
    >Editorial</button>
  </div>

  {#if !previewFilter}
    <div class="tree">
      <!-- The index-wide view — the base of everything. -->
      <div
        class="lib-row"
      class:current={isLibrary}
      class:drop-target={root && dropTarget === root}
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
      {@render statusDot(isLibrary, true)}
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
          class="section-main"
          class:current={photos ? applePhotos?.active && !applePhotos.album : curDir === cat}
          class:import-dest={isImportDest(cat)}
          class:import-branch={isImportBranch(cat)}
          title={photos ? "Apple Photos" : cat}
          aria-pressed={photos ? applePhotos?.active && !applePhotos.album : curDir === cat}
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
      <div class="theme-row">
        <span class="theme-row-label">Theme</span>
        <div class="theme-select-wrap">
          <select
            value={currentThemeId}
            onchange={onThemePick}
            disabled={!curDir}
            aria-label="Folder theme"
          >
            <option value="">Default</option>
            {#each THEMES as t}
              <option value={t.id}>{t.label}</option>
            {/each}
          </select>
          <Icon name="caret-down" size="9px" class="select-caret" />
        </div>
      </div>

      <div class="customize-block">
        <button
          type="button"
          class="customize-toggle"
          onclick={() => (customizeOpen = !customizeOpen)}
          aria-expanded={customizeOpen}
          disabled={!curDir}
        >
          <span>Customize colors & fonts</span>
          <span class="toggle-arrow" class:open={customizeOpen}>
            <Icon name="caret-right" size="9px" />
          </span>
        </button>
        {#if customizeOpen}
          <div class="customize-fields">
            <div class="field-row">
              <span class="field-label">BACKGROUND</span>
              <div class="color-picker-badge">
                <label class="swatch-button" style="background: {previewBg};">
                  <input
                    type="color"
                    value={previewBg}
                    oninput={(e) => applyTheme({ darkBackground: e.currentTarget.value })}
                  />
                </label>
                <span class="hex-text">{previewBg.toUpperCase()}</span>
                {#if storyTheme.darkBackground}
                  <button
                    type="button"
                    class="field-reset"
                    title="Reset to default background"
                    onclick={() => applyTheme({ darkBackground: null })}
                  >
                    <Icon name="x" size="10px" />
                  </button>
                {/if}
              </div>
            </div>
            <div class="field-row">
              <span class="field-label">ACCENT</span>
              <div class="color-picker-badge">
                <label class="swatch-button" style="background: {previewAccent};">
                  <input
                    type="color"
                    value={previewAccent}
                    oninput={(e) => applyTheme({ darkAccent: e.currentTarget.value })}
                  />
                </label>
                <span class="hex-text">{previewAccent.toUpperCase()}</span>
                {#if storyTheme.darkAccent}
                  <button
                    type="button"
                    class="field-reset"
                    title="Reset to default accent"
                    onclick={() => applyTheme({ darkAccent: null })}
                  >
                    <Icon name="x" size="10px" />
                  </button>
                {/if}
              </div>
            </div>
            <div class="field-row">
              <span class="field-label">HEADERS</span>
              <div class="custom-select-wrap">
                <select
                  value={storyTheme.fontHeader ?? ""}
                  onchange={(e) => applyTheme({ fontHeader: e.currentTarget.value || null })}
                >
                  {#each FONTS as f}
                    <option value={f.value ?? ""}>{f.label}</option>
                  {/each}
                </select>
                <Icon name="caret-down" size="9px" class="select-caret" />
              </div>
            </div>
            <div class="field-row">
              <span class="field-label">BODY</span>
              <div class="custom-select-wrap">
                <select
                  value={storyTheme.fontText ?? ""}
                  onchange={(e) => applyTheme({ fontText: e.currentTarget.value || null })}
                >
                  {#each FONTS as f}
                    <option value={f.value ?? ""}>{f.label}</option>
                  {/each}
                </select>
                <Icon name="caret-down" size="9px" class="select-caret" />
              </div>
            </div>
          </div>
        {/if}
      </div>

      <StoryNotesList
        pinned={pinnedStories}
        recent={recentStories}
        {curDir}
        onOpen={onOpenDir}
        onUnpin={unpinStory}
        onPin={pinStory}
        onReorder={onReorderPinned}
      />
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
                  <span>Publishing…</span>
                {:else}
                  <Icon name="arrow-square-out" size="11px" />
                  <span>Publish to Garden</span>
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
    <button class="chrome-btn" onclick={onShowSettings} title="Reveal settings">
      <Icon name="gear" size="12px" />
    </button>
  </div>
</nav>

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

    {#if folderMenu?.path !== root}
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
  {/snippet}
  </ContextMenu>
{/if}

<style>
  /* The one elevated surface — rounded, hairline, soft shadow, floating
     over the recessed grain base with a uniform 8px inset, full height. */
  nav {
    width: 224px;
    flex-shrink: 0;
    margin: 8px;
    background: var(--color-surface-high);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.22);
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
  nav.floating {
    height: min(480px, calc(100vh - 60px));
    margin: 0;
  }
  nav.floating .brand {
    display: none;
  }

  .brand {
    height: 34px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    /* Cancel the card's 8px top inset so these controls stay on the same
       window-chrome baseline as the rail when the sidebar is hidden. */
    transform: translateY(-4px);
    /* Native traffic lights overlay the window top-left — start past them. */
    padding: 0 16px 0 var(--window-controls-offset-sidebar, 78px);
  }
  .chrome-btn {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 12px;
    height: 12px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .chrome-btn:hover {
    color: var(--color-foreground);
  }
  .chrome-btn.on {
    color: var(--color-accent);
  }
  /* circle.circle — a ring with a centred dot, accent when focus is on. */
  .focus-glyph {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1px solid currentColor;
    position: relative;
  }
  .focus-glyph::after {
    content: "";
    position: absolute;
    inset: 2px;
    border-radius: 50%;
    border: 1px solid currentColor;
  }
  .focus-glyph.on::after {
    background: currentColor;
  }
  .wordmark {
    all: unset;
    cursor: pointer;
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .wordmark:hover {
    color: var(--color-foreground);
  }

  .tabs {
    display: flex;
    gap: 2px;
    padding: 0px 12px 6px;
    flex-shrink: 0;
  }
  .tab {
    all: unset;
    cursor: pointer;
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
    padding: 2px 8px;
    border-radius: 999px;
  }
  .tab.active {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 10%, transparent);
  }
  .tab:disabled {
    color: color-mix(in srgb, var(--color-foreground) 18%, transparent);
    cursor: default;
  }

  .tree {
    flex: 1;
    overflow-y: auto;
    padding: 8px 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .lib-row,
  .dir-row {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .lib-row {
    gap: 8px;
    padding: 4px 8px;
  }
  .lib-row.current {
    background: color-mix(in srgb, var(--color-foreground) 10%, transparent);
  }
  .lib-row:hover:not(.current) {
    background: color-mix(in srgb, var(--color-foreground) 5%, transparent);
  }
  .lib-label {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lib-row.current .lib-label {
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
  }
  .indexing {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--color-accent);
  }
  .idx-count {
    font-family: var(--font-monospace, monospace);
    font-size: 9px;
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
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: var(--radius-sm);
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .add-btn:hover {
    color: var(--color-foreground);
  }
  .add-btn:disabled {
    cursor: default;
    opacity: 0.4;
  }

  .section {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px 2px;
    margin-top: 4px;
  }
  .cat-disc {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
    transition: color var(--duration-instant);
  }
  .cat-disc:hover {
    color: var(--color-foreground);
  }
  .section-main {
    all: unset;
    min-width: 0;
    cursor: pointer;
  }
  .section-name {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .section-main.current .section-name,
  .section-main:hover .section-name {
    color: var(--color-foreground);
  }

  .dir-row {
    gap: 8px;
    padding: 4px 8px 4px calc(8px + var(--depth) * 12px);
    position: relative;
  }
  .dir-row.current {
    background: color-mix(in srgb, var(--color-foreground) 12%, transparent);
  }
  .dir-row:hover:not(.current) {
    background: color-mix(in srgb, var(--color-foreground) 6%, transparent);
  }
  /* A photo (or another folder) is being dragged over this folder — it will
     land here on drop. */
  .dir-row.drop-target,
  .lib-row.drop-target {
    background: color-mix(in srgb, var(--color-accent) 18%, transparent);
    box-shadow: inset 0 0 0 1px var(--color-accent);
  }
  /* This folder is the one currently being dragged — it stays put (folders
     aren't reordered by dragging, only moved into another), just quieted so
     the row you're dragging FROM doesn't visually compete with the target. */
  .dir-row.dragging {
    opacity: 0.4;
  }
  /* Inline rename / new-folder editor — same footprint as .dir-name so the
     row doesn't jump when it switches between text and input. */
  .dir-rename {
    all: unset;
    flex: 1;
    min-width: 0;
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-accent) 50%, transparent);
    border-radius: var(--radius-sm);
    padding: 1px 4px;
  }
  .disc {
    all: unset;
    cursor: pointer;
    width: 8px;
    height: 8px;
    box-sizing: content-box;
    /* The visible chevron stays 8px, but an 8x8 hit target is easy to miss
       by a couple pixels — a near-miss lands on .dir-row instead, which
       only ever EXPANDS (never collapses) and no-ops when the folder is
       already current, so the click appeared to do nothing (reproduced
       2026-08-04). Padding widens the clickable area to 20x20 without
       shifting layout — the matching negative margin cancels the padding's
       footprint, so siblings sit exactly where they did before. */
    padding: 6px;
    margin: -6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: color-mix(in srgb, var(--color-foreground) 28%, transparent);
  }
  .disc:hover,
  .disc.open {
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .disc :global(.icon) {
    transition: transform var(--duration-instant) var(--ease-standard);
  }
  .disc.open :global(.icon) {
    transform: rotate(90deg);
  }
  span.disc {
    cursor: default;
  }

  /* The status dot — filled = the folder you're viewing, hollow otherwise;
     the library root carries the Leica red. */
  .dot {
    width: 6.5px;
    height: 6.5px;
    border-radius: 50%;
    border: 1px solid color-mix(in srgb, var(--color-foreground) 25%, transparent);
    flex-shrink: 0;
  }
  .dot.filled {
    width: 7.5px;
    height: 7.5px;
    border: none;
    background: var(--color-foreground);
  }
  .dot.accent {
    border-color: color-mix(in srgb, var(--color-accent) 70%, transparent);
  }
  .dot.accent.filled {
    background: var(--color-accent);
    box-shadow: 0 0 4px color-mix(in srgb, var(--color-accent) 50%, transparent);
  }

  .dir-name {
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    color: color-mix(in srgb, var(--color-foreground) 60%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
    transition: color var(--duration-instant);
  }
  .dir-row.current .dir-name {
    color: var(--color-foreground);
    font-weight: 600;
  }
  .dir-row:hover .dir-name {
    color: var(--color-foreground);
  }
  .dir-spacer {
    flex: 1;
  }
  .dir-count {
    font-family: var(--font-monospace, monospace);
    font-size: 9px;
    font-variant-numeric: tabular-nums;
    color: color-mix(in srgb, var(--color-foreground) 28%, transparent);
    flex-shrink: 0;
    /* Right-aligned in a fixed slot so every count shares one right edge. */
    min-width: 2.4em;
    text-align: right;
  }
  /* Fixed slot for the story marker — present on every row (empty or not) so it
     never shifts the count. */
  .story-slot {
    width: 5px;
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
    padding: 8px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .lib-toggle {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .lib-toggle:hover {
    color: var(--color-foreground);
  }
  .lib-note {
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 6px;
    resize: vertical;
  }
  .lib-open-note {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .lib-open-note:hover {
    color: var(--color-foreground);
  }

  .divider {
    height: 1px;
    background: var(--color-border);
    margin: 0 12px;
    flex-shrink: 0;
  }

  .action-btn.open-page {
    margin-top: 8px;
    width: 100%;
    background: transparent;
    border: 1px solid var(--color-accent);
    color: var(--color-accent);
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.08em;
    padding: 6px 10px;
    border-radius: var(--radius);
    cursor: pointer;
    text-align: center;
    transition: all 0.15s var(--ease-standard);
  }
  .action-btn.open-page:hover {
    background: var(--color-accent);
    color: #ffffff;
  }

  .account {
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
  }
  .account-id {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .account-id:hover {
    color: var(--color-foreground);
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
    color: var(--color-accent);
    font-family: var(--font-header, sans-serif);
    font-size: 9px;
    letter-spacing: 0.06em;
  }
  .account-name {
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .account-form {
    width: 196px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
  }
  .pop-title {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .pop-or {
    text-align: center;
    font-family: var(--font-monospace, monospace);
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 38%, transparent);
  }
  .pop-hint {
    font-family: var(--font-text, sans-serif);
    font-size: 10px;
    line-height: 1.4;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .pop-key {
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 6px;
    outline: none;
  }
  .pop-key:focus {
    border-color: color-mix(in srgb, var(--color-foreground) 30%, transparent);
  }
  .pop-primary {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    text-align: center;
    padding: 6px 8px;
    border-radius: 999px;
    background: var(--color-foreground);
    color: var(--color-surface-high);
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .pop-primary:disabled {
    cursor: default;
    opacity: 0.35;
  }
  .pop-user {
    font-family: var(--font-text, sans-serif);
    font-size: 12px;
    color: var(--color-foreground);
  }
  .pop-meta {
    font-family: var(--font-monospace, monospace);
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .pop-divider {
    height: 1px;
    background: var(--color-border);
  }
  .pop-danger {
    all: unset;
    cursor: pointer;
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--color-accent);
  }

  /* Editorial filter body (THEME + PINNED + RECENT) — replaces the folder
     tree + Library when previewFilter is on. */
  .story-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 0 14px;
    flex: 1;
    overflow-y: auto;
  }
  /* Pushes Publish/Develop/Export to the bottom of the panel when Pinned +
     Recent don't fill it; collapses to 0 and just falls in reading order
     right after them once the list is long enough to scroll. */
  .story-spacer {
    flex: 1;
  }
  .theme-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .theme-row-label {
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    opacity: 0.55;
  }
  .theme-select-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-width: 0;
    flex: 1;
    max-width: 60%;
  }
  .custom-select-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .theme-select-wrap select,
  .custom-select-wrap select {
    appearance: none;
    -webkit-appearance: none;
    width: 100%;
    background: var(--color-surface-high, #222);
    color: var(--color-foreground);
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.12));
    border-radius: var(--radius-sm, 6px);
    padding: 3px 20px 3px 8px;
    font-size: 10px;
    cursor: pointer;
    outline: none;
    font-family: inherit;
    transition: border-color 0.15s ease;
  }
  .custom-select-wrap select {
    width: 120px;
  }
  .theme-select-wrap select:hover,
  .custom-select-wrap select:hover {
    border-color: rgba(255, 255, 255, 0.25);
  }
  .theme-select-wrap select:disabled {
    opacity: 0.4;
    cursor: default;
  }
  :global(.theme-select-wrap .select-caret),
  :global(.custom-select-wrap .select-caret) {
    position: absolute;
    right: 7px;
    pointer-events: none;
    opacity: 0.6;
  }

  /* Fine-tune override toggle — Francis: "the panel should offer the
     ability to customize the style tokens like fonts and color." Kept
     collapsed by default so the plain dropdown stays the primary control. */
  .customize-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .customize-toggle {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    opacity: 0.7;
    transition: opacity 0.15s ease;
    padding: 2px 0;
  }
  .customize-toggle:hover {
    opacity: 1;
  }
  .customize-toggle:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .toggle-arrow {
    margin-left: auto;
    transition: transform 0.18s ease;
  }
  .toggle-arrow.open {
    transform: rotate(90deg);
  }
  .customize-fields {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 6px 0;
  }
  .field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .field-label {
    font-size: 9.5px;
    letter-spacing: 0.08em;
    opacity: 0.6;
    font-weight: 500;
  }
  .color-picker-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--color-surface-high, #222);
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
    border-radius: var(--radius-sm, 6px);
    padding: 2px 6px 2px 3px;
  }
  .swatch-button {
    position: relative;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.25);
    cursor: pointer;
    overflow: hidden;
    display: inline-block;
  }
  .swatch-button input[type="color"] {
    position: absolute;
    top: -20px;
    left: -20px;
    width: 60px;
    height: 60px;
    opacity: 0;
    cursor: pointer;
  }
  .hex-text {
    font-family: var(--font-monospace, monospace);
    font-size: 9.5px;
    opacity: 0.85;
  }
  .field-reset {
    all: unset;
    cursor: pointer;
    opacity: 0.5;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1px;
  }
  .field-reset:hover {
    opacity: 1;
    color: var(--color-accent);
  }
  .theme-inner {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .story-actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .publish-hero-btn {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    width: 100%;
    padding: 7px 12px;
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    border-radius: var(--radius-sm, 6px);
    cursor: pointer;
    background: var(--color-accent, #d6202c);
    color: #ffffff;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    transition: all 0.15s var(--ease-standard);
  }
  .publish-hero-btn:hover:not(:disabled) {
    filter: brightness(1.1);
    box-shadow: 0 4px 12px rgba(214, 32, 44, 0.35);
  }
  .publish-hero-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .secondary-actions {
    display: flex;
    gap: 6px;
  }
  .action-btn.secondary {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 5px 8px;
    font-size: 9.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    border-radius: var(--radius-sm, 6px);
    cursor: pointer;
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.12));
    background: var(--color-surface-low, rgba(255, 255, 255, 0.04));
    color: var(--color-foreground);
    transition: all 0.15s ease;
  }
  .action-btn.secondary:hover:not(:disabled) {
    background: var(--color-surface-high, rgba(255, 255, 255, 0.1));
    border-color: rgba(255, 255, 255, 0.25);
  }
  .action-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .open-page-banner {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background: color-mix(in srgb, #4caf50 12%, transparent);
    border: 1px solid color-mix(in srgb, #4caf50 35%, transparent);
    border-radius: var(--radius-sm, 6px);
    cursor: pointer;
    font-size: 10px;
    color: #4caf50;
    font-weight: 500;
    transition: all 0.15s ease;
  }
  .open-page-banner:hover {
    background: color-mix(in srgb, #4caf50 20%, transparent);
    border-color: #4caf50;
  }
  :global(.banner-check) {
    color: #4caf50;
  }
  :global(.banner-arrow) {
    margin-left: auto;
    opacity: 0.8;
  }
  .publish-status {
    font-size: 10px;
    opacity: 0.6;
    margin: 0;
  }
</style>
