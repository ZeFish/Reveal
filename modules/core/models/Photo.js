/**
 * Domain model and functional namespace for Photos and Frames in Reveal.
 *
 * Provides zero-allocation static utilities for high-throughput contexts
 * (e.g. virtual grid of 20,000 frames) and an object-oriented class for
 * focused photo views (Develop mode, Loupe, Export, Story).
 */

const RAW_EXTENSIONS = new Set([
  "raf",
  "cr2",
  "cr3",
  "arw",
  "nef",
  "dng",
  "rw2",
  "orf",
  "pef",
  "3fr",
  "iiq",
  "srw",
  "crw",
  "gpr",
]);

/**
 * @typedef {Object} FrameData
 * @property {string} path
 * @property {string} [name]
 * @property {number} [previewVersion]
 * @property {number} [rating]
 * @property {number} [width]
 * @property {number} [height]
 * @property {string | number} [capture_at]
 * @property {string} [captured_at]
 * @property {number} [aperture]
 * @property {string} [shutter]
 * @property {number} [iso]
 * @property {number} [focal_mm]
 * @property {string} [make]
 * @property {string} [model]
 */

export class Photo {
  /**
   * @param {FrameData} data
   */
  constructor(data) {
    /** @type {string} */
    this.path = data.path;
    /** @type {string} */
    this.name = data.name || Photo.stem(data.path);
    /** @type {number} */
    this.previewVersion = data.previewVersion ?? 0;
    /** @type {number} */
    this.rating = data.rating ?? 0;
    /** @type {number | undefined} */
    this.width = data.width;
    /** @type {number | undefined} */
    this.height = data.height;
    /** @type {string | number | undefined} */
    this.capture_at = data.capture_at ?? data.captured_at;
    /** @type {number | undefined} */
    this.aperture = data.aperture;
    /** @type {string | undefined} */
    this.shutter = data.shutter;
    /** @type {number | undefined} */
    this.iso = data.iso;
    /** @type {number | undefined} */
    this.focal_mm = data.focal_mm;
    /** @type {string | undefined} */
    this.make = data.make;
    /** @type {string | undefined} */
    this.model = data.model;
  }

  /**
   * Factory returning a Photo instance, or null if input is falsy.
   * @param {FrameData | Photo | null | undefined} raw
   * @returns {Photo | null}
   */
  static from(raw) {
    if (!raw) return null;
    if (raw instanceof Photo) return raw;
    return new Photo(raw);
  }

  /**
   * Builds the custom `reveal://thumb` URI for a photo or path.
   *
   * @param {string | FrameData | Photo | null | undefined} photoOrPath
   * @param {Object} [options]
   * @param {number} [options.size=768]
   * @param {number} [options.version]
   * @param {number | null} [options.attempt]
   * @param {boolean} [options.priority]
   * @returns {string}
   */
  static thumb(photoOrPath, options = {}) {
    if (!photoOrPath) return "";

    const path = typeof photoOrPath === "string" ? photoOrPath : photoOrPath.path;
    if (!path) return "";

    const size = options.size ?? 768;
    const version = options.version ?? (typeof photoOrPath === "object" ? (photoOrPath.previewVersion ?? 0) : 0);

    let url = `reveal://thumb?p=${encodeURIComponent(path)}&v=${version}&size=${size}`;
    if (options.attempt) {
      url += `&r=${options.attempt}`;
    }
    if (options.priority) {
      url += `&priority=1`;
    }
    return url;
  }

  /**
   * Extracts the base stem of a filename without extension.
   * Handles `<stem>.reveal.jpg` as well as standard extensions.
   *
   * @param {string | FrameData | Photo | null | undefined} photoOrPath
   * @returns {string}
   */
  static stem(photoOrPath) {
    if (!photoOrPath) return "";
    let s = typeof photoOrPath === "string" ? photoOrPath : (photoOrPath.name || photoOrPath.path || "");
    const slash = s.lastIndexOf("/");
    if (slash !== -1) s = s.slice(slash + 1);
    const dot = s.lastIndexOf(".");
    if (dot > 0) s = s.slice(0, dot);
    if (s.endsWith(".reveal")) s = s.slice(0, -".reveal".length);
    return s;
  }

  /**
   * Checks whether the file corresponds to a RAW camera format.
   * @param {string | FrameData | Photo | null | undefined} photoOrPath
   * @returns {boolean}
   */
  static isRaw(photoOrPath) {
    if (!photoOrPath) return false;
    const s = typeof photoOrPath === "string" ? photoOrPath : (photoOrPath.path || photoOrPath.name || "");
    const dot = s.lastIndexOf(".");
    if (dot === -1) return false;
    const ext = s.slice(dot + 1).toLowerCase();
    return RAW_EXTENSIONS.has(ext);
  }

  /**
   * Computes aspect ratio (width / height) with a fallback.
   * @param {FrameData | Photo | null | undefined} photo
   * @param {number} [fallback=1.5]
   * @returns {number}
   */
  static aspectRatio(photo, fallback = 1.5) {
    if (photo && photo.width && photo.height && photo.height > 0) {
      return photo.width / photo.height;
    }
    return fallback;
  }

  /**
   * Formats exposure settings into a human-readable string:
   * e.g. "ƒ1.4 · 1/250s · ISO 400 · 35mm"
   *
   * @param {FrameData | Photo | null | undefined} photo
   * @returns {string}
   */
  static formatExposure(photo) {
    if (!photo) return "";
    const parts = [];
    if (photo.aperture) parts.push(`ƒ${Number(photo.aperture.toFixed(1))}`);
    if (photo.shutter) parts.push(photo.shutter);
    if (photo.iso) parts.push(`ISO ${photo.iso}`);
    if (photo.focal_mm) parts.push(`${Math.round(photo.focal_mm)}mm`);
    return parts.join(" · ");
  }

  // --- Instance Getters & Methods ---

  /** @returns {string} */
  get thumb() {
    return Photo.thumb(this, { size: 768 });
  }

  /** @returns {string} */
  get largeThumb() {
    return Photo.thumb(this, { size: 2048 });
  }

  /** @returns {string} */
  get stem() {
    return Photo.stem(this);
  }

  /** @returns {boolean} */
  get isRaw() {
    return Photo.isRaw(this);
  }

  /** @returns {number} */
  get aspectRatio() {
    return Photo.aspectRatio(this);
  }

  /** @returns {string} */
  get exposure() {
    return Photo.formatExposure(this);
  }

  /**
   * Custom thumb URL for this photo instance.
   * @param {Omit<Parameters<typeof Photo.thumb>[1], "version">} [options]
   */
  thumbUrl(options = {}) {
    return Photo.thumb(this, options);
  }
}
