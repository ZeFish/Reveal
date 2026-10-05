/**
 * Domain model representing a Photo Library Source in Reveal.
 *
 * A Source is any provider of media:
 * - Local filesystem folder / NAS volume ("folder")
 * - Apple Photos PhotoKit library ("apple-photos")
 * - Remote Immich server instance ("immich")
 * - Virtual / Smart collection root ("virtual")
 */

/**
 * @typedef {"folder" | "apple-photos" | "immich" | "virtual"} SourceType
 * @typedef {"idle" | "scanning" | "syncing" | "error"} SourceStatus
 *
 * @typedef {Object} SourceData
 * @property {string} id
 * @property {SourceType} type
 * @property {string} name
 * @property {string} [path]
 * @property {string} [icon]
 * @property {boolean} [connected=true]
 * @property {SourceStatus} [status="idle"]
 * @property {boolean} [readOnly=false]
 * @property {number} [totalPhotos]
 */

export class Source {
  /**
   * @param {SourceData} data
   */
  constructor(data) {
    /** @type {string} */
    this.id = data.id;
    /** @type {SourceType} */
    this.type = data.type;
    /** @type {string} */
    this.name = data.name;
    /** @type {string | undefined} */
    this.path = data.path;
    /** @type {string} */
    this.icon = data.icon || (data.type === "apple-photos" ? "image" : data.type === "immich" ? "cloud" : "folder");
    /** @type {boolean} */
    this.connected = data.connected ?? true;
    /** @type {SourceStatus} */
    this.status = data.status || "idle";
    /** @type {boolean} */
    this.readOnly = data.readOnly ?? false;
    /** @type {number | undefined} */
    this.totalPhotos = data.totalPhotos;
  }

  /**
   * Creates a Source instance from a local root directory path.
   *
   * @param {string} rootPath
   * @param {number} [totalPhotos]
   * @returns {Source}
   */
  static fromLocalFolder(rootPath, totalPhotos = 0) {
    const cleanPath = rootPath.replace(/\/+$/, "");
    const name = cleanPath.split("/").pop() || cleanPath;
    return new Source({
      id: `local:${cleanPath}`,
      type: "folder",
      name,
      path: cleanPath,
      icon: "folder",
      connected: true,
      readOnly: false,
      totalPhotos,
    });
  }

  /**
   * Creates a Source instance representing the macOS Apple Photos library.
   *
   * @param {{ supported?: boolean, busy?: boolean, active?: boolean, total?: number | null }} [state]
   * @returns {Source}
   */
  static fromApplePhotos(state = {}) {
    return new Source({
      id: "apple-photos",
      type: "apple-photos",
      name: "Apple Photos",
      icon: "image",
      connected: Boolean(state.supported),
      status: state.busy ? "syncing" : "idle",
      readOnly: true,
      totalPhotos: state.total ?? undefined,
    });
  }

  /**
   * Creates a Source instance representing an Immich server.
   *
   * @param {{ connected?: boolean, busy?: boolean, active?: boolean, libraryTotal?: number | null, total?: number | null }} [state]
   * @returns {Source}
   */
  static fromImmich(state = {}) {
    return new Source({
      id: "immich",
      type: "immich",
      name: "Immich",
      icon: "cloud",
      connected: Boolean(state.connected),
      status: state.busy ? "syncing" : "idle",
      readOnly: true,
      totalPhotos: state.libraryTotal ?? state.total ?? undefined,
    });
  }

  /**
   * Whether this source represents a local filesystem folder.
   * @returns {boolean}
   */
  get isLocal() {
    return this.type === "folder";
  }

  /**
   * Whether this source is a remote or read-only provider.
   * @returns {boolean}
   */
  get isRemote() {
    return this.type === "apple-photos" || this.type === "immich";
  }
}
