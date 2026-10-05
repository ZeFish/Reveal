/**
 * Tree operations and persistence helpers for the folder browser sidebar.
 */

export const EXPANDED_KEY = "reveal.sidebar.expanded.v2";
export const MANUALLY_COLLAPSED_KEY = "reveal.sidebar.manuallyCollapsed.v2";
export const CAT_EXPANDED_KEY = "reveal.sidebar.catalogs.expanded.v2";

/**
 * Reads a Set of string keys from localStorage safely.
 * @param {string} key
 * @returns {Set<string>}
 */
export function readPersistedSet(key) {
  if (typeof localStorage === "undefined") return new Set();
  try {
    const saved = JSON.parse(localStorage.getItem(key) ?? "null");
    if (Array.isArray(saved)) return new Set(saved);
  } catch (_) {}
  return new Set();
}

/**
 * Persists a Set of string keys to localStorage safely.
 * @param {string} key
 * @param {Set<string>} set
 */
export function writePersistedSet(key, set) {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(key, JSON.stringify([...set]));
  } catch (_) {}
}

/**
 * Filters out ghost folders whose paths are now present in the indexed directories.
 * @param {Array<{ abs: string, name: string, parentAbs: string }>} ghosts
 * @param {Array<{ dir: string, count: number }>} dirs
 * @returns {Array<{ abs: string, name: string, parentAbs: string }>}
 */
export function filterGhosts(ghosts, dirs) {
  const real = new Set((dirs || []).map((d) => d.dir));
  return (ghosts || []).filter((g) => !real.has(g.abs));
}

/**
 * @typedef {Object} TreeNode
 * @property {string} name
 * @property {string} rel
 * @property {string} abs
 * @property {number} count
 * @property {TreeNode[]} children
 */

/**
 * @typedef {Object} CatalogueTree
 * @property {string} cat
 * @property {string} name
 * @property {number} total
 * @property {{ nodes: TreeNode[] }} tree
 */

/**
 * Builds the hierarchical folder trees for each registered catalogue root.
 *
 * @param {Object} options
 * @param {string[]} [options.roots]
 * @param {string | null} [options.root]
 * @param {Array<{ dir: string, count: number }>} [options.dirs]
 * @param {(cat: string) => boolean} [options.isCatExpanded]
 * @returns {CatalogueTree[]}
 */
export function buildCatalogueTrees({
  roots = [],
  root = null,
  dirs = [],
  isCatExpanded = () => true,
}) {
  const rootsList = roots?.length ? [...roots] : root ? [root] : [];
  return rootsList.map((catRoot) => {
    let total = 0;
    const isExpanded = isCatExpanded(catRoot);
    /** @type {TreeNode[]} */
    const top = [];

    if (!isExpanded) {
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
            node = {
              name: part,
              rel: `${catRoot}/${acc}`,
              abs: `${catRoot}/${acc}`,
              count: 0,
              children: [],
            };
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
}

