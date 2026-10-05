/** @typedef {{id: string, title: string, kind: "folder" | "album", count: number | null, children: PhotoCollection[]}} PhotoCollection */
/** @typedef {{supported: boolean, active: boolean, busy: boolean, loaded: boolean, album: string, albums: PhotoCollection[], total: number | null}} PhotoLibrary */

export const APPLE_PHOTOS_ROOT = "apple-photos-album:";

/**
 * PhotoKit identifiers, not titles, own identity and expansion state. Album
 * titles can repeat and can change independently of the selected collection.
 * @param {PhotoCollection[]} collections
 * @param {string} id
 * @returns {PhotoCollection | undefined}
 */
export function findPhotoCollection(collections, id) {
  for (const collection of collections) {
    if (collection.id === id) return collection;
    const child = findPhotoCollection(collection.children, id);
    if (child) return child;
  }
}

/**
 * @param {PhotoCollection[]} collections
 * @param {string} id
 * @returns {string[]}
 */
export function photoCollectionAncestors(collections, id) {
  for (const collection of collections) {
    if (collection.id === id) return [APPLE_PHOTOS_ROOT + id];
    const descendants = photoCollectionAncestors(collection.children, id);
    if (descendants.length) return [APPLE_PHOTOS_ROOT + collection.id, ...descendants];
  }
  return [];
}
