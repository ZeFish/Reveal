/**
 * Virtual & Curated Collections store.
 *
 * Manages smart queries (Favorites, Picks, Stories) and curated photo sets
 * that support drag-and-drop organisation without moving files on disk.
 */

import { Collection } from "../models/Collection.js";

const STORAGE_KEY = "reveal.virtual_collections.v1";

/**
 * @typedef {Object} SerializedCuratedCollection
 * @property {string} id
 * @property {string} name
 * @property {string} [icon]
 * @property {string[]} uris
 */

/**
 * Reads saved custom collections from localStorage safely.
 * @returns {SerializedCuratedCollection[]}
 */
function readSavedCollections() {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch (_) {
    return [];
  }
}

/**
 * Persists custom collections to localStorage.
 * @param {SerializedCuratedCollection[]} collections
 */
function writeSavedCollections(collections) {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(collections));
  } catch (_) {}
}

/** @type {SerializedCuratedCollection[]} */
let userCollections = $state(readSavedCollections());

/**
 * Default Smart Collections.
 */
export const SMART_COLLECTIONS = [
  Collection.fromVirtual({
    id: "virtual://favorites",
    name: "Favorites 5★",
    icon: "star",
    query: { filter: { minRating: 5 } },
  }),
  Collection.fromVirtual({
    id: "virtual://picks",
    name: "Picked",
    icon: "flag",
    query: { filter: { pick: "picked" } },
  }),
  Collection.fromVirtual({
    id: "virtual://story",
    name: "Story Board",
    icon: "newspaper",
    query: { filter: { storyOnly: true } },
  }),
];

export const virtualCollections = {
  /**
   * All active virtual collections (smart + curated).
   * @returns {Collection[]}
   */
  get all() {
    const curated = userCollections.map((c) =>
      Collection.fromVirtual({
        id: c.id,
        name: c.name,
        icon: c.icon || "bookmark",
        count: c.uris?.length ?? 0,
        query: { uris: c.uris || [] },
      }),
    );
    return [...SMART_COLLECTIONS, ...curated];
  },

  /**
   * Computes collections with live photo counts for smart queries.
   * @param {any[]} [frames]
   * @param {Set<string>} [storySet]
   * @returns {Collection[]}
   */
  withCounts(frames = [], storySet = new Set()) {
    const favCount = frames.filter((f) => (f.rating ?? 0) >= 5).length;
    const pickCount = frames.filter((f) => f.pick === "picked").length;
    const storyCount = frames.filter((f) => {
      const stem = (f.name || f.path.split("/").pop() || "").replace(/\.[^.]+$/, "");
      return storySet.has(stem);
    }).length;

    const smart = [
      Collection.fromVirtual({
        id: "virtual://favorites",
        name: "Favorites 5★",
        icon: "star",
        count: favCount,
        query: { filter: { minRating: 5 } },
      }),
      Collection.fromVirtual({
        id: "virtual://picks",
        name: "Picked",
        icon: "flag",
        count: pickCount,
        query: { filter: { pick: "picked" } },
      }),
      Collection.fromVirtual({
        id: "virtual://story",
        name: "Story Board",
        icon: "newspaper",
        count: storyCount,
        query: { filter: { storyOnly: true } },
      }),
    ];

    const curated = userCollections.map((c) =>
      Collection.fromVirtual({
        id: c.id,
        name: c.name,
        icon: c.icon || "bookmark",
        count: c.uris?.length ?? 0,
        query: { uris: c.uris || [] },
      }),
    );

    return [...smart, ...curated];
  },

  /**
   * Finds a virtual collection by ID.
   * @param {string} id
   * @returns {Collection | undefined}
   */
  get(id) {
    return this.all.find((c) => c.id === id);
  },

  /**
   * Checks whether an ID represents a virtual collection.
   * @param {string | null | undefined} id
   * @returns {boolean}
   */
  isVirtualId(id) {
    return typeof id === "string" && id.startsWith("virtual://");
  },

  /**
   * Creates a new user-curated collection.
   * @param {string} name
   * @param {string} [icon]
   * @returns {Collection}
   */
  create(name, icon = "bookmark") {
    const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "") || "collection";
    const id = `virtual://curated/${slug}-${Date.now()}`;
    const entry = { id, name, icon, uris: [] };
    userCollections = [...userCollections, entry];
    writeSavedCollections(userCollections);
    return Collection.fromVirtual({ id, name, icon, count: 0, query: { uris: [] } });
  },

  /**
   * Removes a curated collection.
   * @param {string} id
   */
  remove(id) {
    userCollections = userCollections.filter((c) => c.id !== id);
    writeSavedCollections(userCollections);
  },

  /**
   * Adds photos to a curated collection (e.g. on Drag & Drop).
   * @param {string} collectionId
   * @param {string[]} photoPaths
   * @returns {number} count of added photos
   */
  addPhotos(collectionId, photoPaths) {
    let added = 0;
    userCollections = userCollections.map((c) => {
      if (c.id !== collectionId) return c;
      const set = new Set(c.uris || []);
      for (const p of photoPaths) {
        if (!set.has(p)) {
          set.add(p);
          added++;
        }
      }
      return { ...c, uris: Array.from(set) };
    });
    if (added > 0) writeSavedCollections(userCollections);
    return added;
  },

  /**
   * Removes photos from a curated collection.
   * @param {string} collectionId
   * @param {string[]} photoPaths
   */
  removePhotos(collectionId, photoPaths) {
    const toRemove = new Set(photoPaths);
    userCollections = userCollections.map((c) => {
      if (c.id !== collectionId) return c;
      return { ...c, uris: (c.uris || []).filter((p) => !toRemove.has(p)) };
    });
    writeSavedCollections(userCollections);
  },

  /**
   * Filters an array of photos for a given virtual collection.
   * @param {Collection} collection
   * @param {any[]} allFrames
   * @param {Set<string>} [storySet]
   * @returns {any[]}
   */
  filterFrames(collection, allFrames, storySet = new Set()) {
    if (!collection) return allFrames;

    // Explicit URIs (Curated set)
    if (collection.query?.uris) {
      const allowed = new Set(collection.query.uris);
      return allFrames.filter((f) => allowed.has(f.path));
    }

    const filter = collection.query?.filter;
    if (!filter) return allFrames;

    return allFrames.filter((f) => {
      if (filter.minRating !== undefined && (f.rating ?? 0) < filter.minRating) return false;
      if (filter.pick && f.pick !== filter.pick) return false;
      if (filter.storyOnly) {
        const stem = (f.path.split("/").pop() || "").replace(/\.[^.]+$/, "");
        if (!storySet.has(stem)) return false;
      }
      return true;
    });
  },
};
