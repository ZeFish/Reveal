/**
 * What went wrong while Apple Photos handed us a photo, in words a person can act on.
 *
 * Photos reports iCloud trouble as `The operation couldn't be completed.
 * (CloudPhotoLibraryErrorDomain error 1005.)` — a domain and a number, with nothing about what to
 * do. We do not know every code, so the explanation says what is certain (iCloud could not provide
 * the photo just now) and what to try, and the original text stays one hover away.
 *
 * @param {unknown} error
 * @returns {string}
 */
export function explainTransferError(error) {
  const text = String(error ?? "").trim();
  if (/CloudPhotoLibraryErrorDomain|iCloud/i.test(text)) {
    return "iCloud could not provide this photo just now. Check the connection and that iCloud Photos is available, then open it again.";
  }
  if (/denied|not authorized|authoriz/i.test(text)) {
    return "Reveal is not allowed to read Apple Photos. Allow it in System Settings → Privacy & Security → Photos.";
  }
  return text || "Apple Photos could not provide this photo.";
}
