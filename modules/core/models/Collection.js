/**
 * Domain model representing a Container or Collection of Photos in Reveal.
 *
 * A Collection can be:
 * - A physical directory on the local filesystem ("folder")
 * - An album or collection from an external provider ("album" e.g. Apple Photos, Immich)
 * - A virtual smart query or curated selection ("virtual")
 */

/**
 * @typedef {"folder" | "album" | "virtual"} CollectionKind
 *
 * @typedef {Object} CollectionQuery
 * @property {Record<string, any>} [filter] e.g. { rating: { gte: 4 }, isRaw: true }
 * @property {string[]} [uris] explicit photo paths/URIs for curated sets
 * @property {string} [sourceId] optional target source filter
 *
 * @typedef {Object} CollectionData
 * @property {string} id Unique URI (e.g. "/nas/2026/09", "apple-photos://album/xyz", "virtual://favorites")
 * @property {string} sourceId ID of the parent Source (e.g. "local:/nas", "apple-photos", "reveal")
 * @property {string} name Display name
 * @property {CollectionKind} kind
 * @property {string} [path] Filesystem path if local
 * @property {string} [icon]
 * @property {number} [count]
 * @property {string | null} [parentUri]
 * @property {boolean} [readOnly]
 * @property {CollectionQuery} [query]
 */

export class Collection {
  /**
   * @param {CollectionData} data
   */
  constructor(data) {
    /** @type {string} */
    this.id = data.id;
    /** @type {string} */
    this.sourceId = data.sourceId;
    /** @type {string} */
    this.name = data.name;
    /** @type {CollectionKind} */
    this.kind = data.kind;
    /** @type {string | undefined} */
    this.path = data.path;
    /** @type {string} */
    this.icon = data.icon || (data.kind === "virtual" ? "star" : data.kind === "album" ? "image" : "folder");
    /** @type {number} */
    this.count = data.count ?? 0;
    /** @type {string | null} */
    this.parentUri = data.parentUri ?? null;
    /** @type {boolean} */
    this.readOnly = data.readOnly ?? (data.kind !== "folder");
    /** @type {CollectionQuery | undefined} */
    this.query = data.query;
  }

  /**
   * Builds a relative display label for a directory given the configured roots.
   *
   * @param {string | null | undefined} dir
   * @param {string[]} [roots]
   * @returns {string}
   */
  static label(dir, roots = []) {
    if (!dir) return "";
    const cleanDir = dir.replace(/\/+$/, "");
    const owner = roots.find((r) => cleanDir === r || cleanDir.startsWith(r.replace(/\/+$/, "") + "/"));
    if (owner) {
      const cleanOwner = owner.replace(/\/+$/, "");
      const relative = cleanDir.slice(cleanOwner.length).replace(/^\/+/, "");
      return relative || cleanOwner.split("/").pop() || cleanOwner;
    }
    return cleanDir.split("/").pop() || cleanDir;
  }

  /**
   * Returns the immediate parent directory path, or null if root / invalid.
   * @param {string | null | undefined} dir
   * @returns {string | null}
   */
  static parentOf(dir) {
    if (!dir) return null;
    const clean = dir.replace(/\/+$/, "");
    const idx = clean.lastIndexOf("/");
    if (idx <= 0) return null;
    return clean.slice(0, idx);
  }

  /**
   * Generates the list of ancestor paths from root down to this directory.
   * @param {string} dir
   * @param {string} [root]
   * @returns {string[]}
   */
  static ancestors(dir, root = "") {
    const list = [];
    let cur = Collection.parentOf(dir);
    const stopAt = root ? root.replace(/\/+$/, "") : "";
    while (cur && (!stopAt || cur.startsWith(stopAt))) {
      list.unshift(cur);
      if (cur === stopAt) break;
      cur = Collection.parentOf(cur);
    }
    return list;
  }

  /**
   * Checks whether a directory path matches one of the root directories.
   * @param {string} dir
   * @param {string[]} [roots]
   * @returns {boolean}
   */
  static isRoot(dir, roots = []) {
    const clean = dir.replace(/\/+$/, "");
    return roots.some((r) => r.replace(/\/+$/, "") === clean);
  }

  /**
   * Returns the canonical story markdown note path for a given local folder.
   * @param {string} dir
   * @returns {string}
   */
  static storyPath(dir) {
    const clean = dir.replace(/\/+$/, "");
    return `${clean}/.reveal/story.md`;
  }

  /**
   * Factory for local filesystem directory collections.
   *
   * @param {string} dirPath
   * @param {string[]} [roots]
   * @param {number} [count=0]
   * @returns {Collection}
   */
  static fromFolder(dirPath, roots = [], count = 0) {
    const clean = dirPath.replace(/\/+$/, "");
    const owner = roots.find((r) => clean === r || clean.startsWith(r.replace(/\/+$/, "") + "/"));
    const sourceId = owner ? `local:${owner.replace(/\/+$/, "")}` : "local:root";
    const name = Collection.label(clean, roots);
    const parentUri = Collection.parentOf(clean);

    return new Collection({
      id: clean,
      sourceId,
      name,
      kind: "folder",
      path: clean,
      icon: "folder",
      count,
      parentUri,
      readOnly: false,
    });
  }

  /**
   * Factory for Apple Photos albums.
   *
   * @param {{ id: string, title?: string, name?: string, count?: number }} album
   * @returns {Collection}
   */
  static fromAppleAlbum(album) {
    const id = album.id.startsWith("apple-photos://") ? album.id : `apple-photos://album/${album.id}`;
    return new Collection({
      id,
      sourceId: "apple-photos",
      name: album.title || album.name || "Album",
      kind: "album",
      icon: "image",
      count: album.count ?? 0,
      readOnly: true,
    });
  }

  /**
   * Factory for Immich remote albums.
   *
   * @param {{ id: string, title?: string, name?: string, count?: number }} album
   * @returns {Collection}
   */
  static fromImmichAlbum(album) {
    const id = album.id.startsWith("immich://") ? album.id : `immich://album/${album.id}`;
    return new Collection({
      id,
      sourceId: "immich",
      name: album.title || album.name || "Album Immich",
      kind: "album",
      icon: "cloud",
      count: album.count ?? 0,
      readOnly: true,
    });
  }

  /**
   * Factory for virtual smart collections or curated reels.
   *
   * @param {{ id?: string, name: string, icon?: string, query?: CollectionQuery, count?: number }} spec
   * @returns {Collection}
   */
  static fromVirtual(spec) {
    const slug = spec.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "");
    const id = spec.id || `virtual://${slug}`;
    return new Collection({
      id,
      sourceId: "reveal",
      name: spec.name,
      kind: "virtual",
      icon: spec.icon || "star",
      count: spec.count ?? 0,
      readOnly: true,
      query: spec.query,
    });
  }

  /** @returns {boolean} */
  get isVirtual() {
    return this.kind === "virtual";
  }

  /** @returns {boolean} */
  get isRemote() {
    return this.sourceId === "apple-photos" || this.sourceId === "immich";
  }

  /** @returns {boolean} */
  get isFolder() {
    return this.kind === "folder";
  }
}
