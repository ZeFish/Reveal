// Thin wrappers around Tauri IPC — one export per Rust command, so the
// components never import @tauri-apps/api directly.
import { invoke } from "@tauri-apps/api/core";

/** True when running inside the Tauri webview (vs plain `vite dev` in a browser). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function ping() {
  if (!isTauri) return Promise.resolve("pong (browser mock)");
  return invoke("ping");
}
