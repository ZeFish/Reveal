/**
 * Domain model representing an Export Destination in Reveal.
 *
 * A Destination can be:
 * - A local directory on the filesystem ("folder")
 * - An Obsidian vault / Daily note ("obsidian")
 * - A Garden web publication ("garden")
 * - An external application editor ("editor", e.g. Photoshop, Affinity)
 * - A remote cloud provider ("remote", e.g. Immich)
 */

/**
 * @typedef {"folder" | "obsidian" | "garden" | "editor" | "remote"} DestinationKind
 *
 * @typedef {Object} DestinationOptions
 * @property {number} [longEdge] Longest dimension in pixels (0 = full resolution)
 * @property {boolean} [border] Add aesthetic white border
 * @property {number} [borderFrac] Fractional border thickness (default 0.05 when border is true)
 * @property {number} [quality] Output quality (0-100)
 * @property {boolean} [allowDownload] Allow visitors to download full-res file (for Garden)
 *
 * @typedef {Object} DestinationData
 * @property {string} id Unique ID for the destination
 * @property {DestinationKind} kind
 * @property {string} name Human-friendly display label
 * @property {string} [icon] Phosphor icon name
 * @property {string} [path] Target filesystem path or app path
 * @property {boolean} [ready] Whether this destination is fully configured and ready
 * @property {DestinationOptions} [options] Default export options for this destination
 */

export class Destination {
  /**
   * @param {DestinationData} data
   */
  constructor(data) {
    /** @type {string} */
    this.id = data.id;
    /** @type {DestinationKind} */
    this.kind = data.kind;
    /** @type {string} */
    this.name = data.name;
    /** @type {string} */
    this.icon = data.icon || Destination.defaultIcon(data.kind);
    /** @type {string | undefined} */
    this.path = data.path;
    /** @type {boolean} */
    this.ready = data.ready ?? true;
    /** @type {DestinationOptions} */
    this.options = {
      longEdge: data.options?.longEdge ?? 2048,
      border: data.options?.border ?? false,
      borderFrac: data.options?.borderFrac ?? 0.05,
      quality: data.options?.quality ?? 92,
      allowDownload: data.options?.allowDownload ?? false,
    };
  }

  /**
   * Default icon for a given destination kind.
   * @param {DestinationKind} kind
   * @returns {string}
   */
  static defaultIcon(kind) {
    switch (kind) {
      case "obsidian":
        return "book-bookmark";
      case "garden":
        return "plant";
      case "editor":
        return "arrow-square-out";
      case "remote":
        return "cloud";
      case "folder":
      default:
        return "folder";
    }
  }

  /**
   * Factory for local filesystem folder destinations.
   *
   * @param {string} [path] Filesystem path (falls back to Desktop if empty)
   * @param {DestinationOptions} [options]
   * @returns {Destination}
   */
  static localFolder(path = "", options = {}) {
    const cleanPath = path ? path.replace(/\/+$/, "") : "";
    const name = cleanPath ? cleanPath.split("/").pop() || "Folder" : "Desktop";
    return new Destination({
      id: cleanPath ? `folder:${cleanPath}` : "folder:desktop",
      kind: "folder",
      name,
      icon: "folder",
      path: cleanPath,
      ready: true,
      options,
    });
  }

  /**
   * Factory for Obsidian Daily Note export.
   *
   * @param {DestinationOptions} [options]
   * @returns {Destination}
   */
  static obsidian(options = {}) {
    return new Destination({
      id: "obsidian:daily-note",
      kind: "obsidian",
      name: "Obsidian Daily Note",
      icon: "book-bookmark",
      ready: true,
      options,
    });
  }

  /**
   * Factory for Garden web publication.
   *
   * @param {DestinationOptions & { gardenUrl?: string, signedIn?: boolean }} [options]
   * @returns {Destination}
   */
  static garden(options = {}) {
    return new Destination({
      id: "garden:publication",
      kind: "garden",
      name: "Garden",
      icon: "plant",
      path: options.gardenUrl,
      ready: options.signedIn ?? true,
      options: {
        ...options,
        allowDownload: options.allowDownload ?? false,
      },
    });
  }

  /**
   * Factory for external app editors (Photoshop, Affinity, Capture One, etc.).
   *
   * @param {string} name App display name
   * @param {string} appPath Filesystem path to application bundle
   * @param {DestinationOptions} [options]
   * @returns {Destination}
   */
  static editor(name, appPath, options = {}) {
    return new Destination({
      id: `editor:${name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`,
      kind: "editor",
      name,
      icon: "arrow-square-out",
      path: appPath,
      ready: Boolean(appPath),
      options,
    });
  }

  /**
   * Factory for Immich remote album destination.
   *
   * @param {{ albumId?: string, albumTitle?: string }} [remote]
   * @param {DestinationOptions} [options]
   * @returns {Destination}
   */
  static immich(remote = {}, options = {}) {
    const title = remote.albumTitle || "Immich";
    const id = remote.albumId ? `immich:album:${remote.albumId}` : "immich:library";
    return new Destination({
      id,
      kind: "remote",
      name: title,
      icon: "cloud",
      ready: true,
      options,
    });
  }

  /** @returns {boolean} */
  get isLocal() {
    return this.kind === "folder";
  }

  /** @returns {boolean} */
  get isObsidian() {
    return this.kind === "obsidian";
  }

  /** @returns {boolean} */
  get isGarden() {
    return this.kind === "garden";
  }

  /** @returns {boolean} */
  get isEditor() {
    return this.kind === "editor";
  }

  /** @returns {boolean} */
  get isRemote() {
    return this.kind === "remote" || this.kind === "garden";
  }
}
