import RapidEngine from "./rapid/RapidEngine.svelte";
import SpektraEngine from "./spektra/SpektraEngine.svelte";
import GenericEngine from "./GenericEngine.svelte";

/**
 * @typedef {Object} EnginePlugin
 * @property {string} id Unique engine ID
 * @property {string} label User-facing display label
 * @property {any} component Svelte component used to render the controls
 * @property {string} [description]
 * @property {string} [icon]
 */

/** @type {Map<string, EnginePlugin>} */
const registry = new Map();

// Built-in standard engines
registry.set("rapid", {
  id: "rapid",
  label: "Rapid",
  component: RapidEngine,
  description: "GPU-accelerated digital engine with curves, AgX looks, and LUTs",
  icon: "zap",
});

registry.set("spektra", {
  id: "spektra",
  label: "Spektra",
  component: SpektraEngine,
  description: "Analog darkroom simulation with authentic film and paper emulsions",
  icon: "film",
});

/**
 * Registers or overrides an engine plugin.
 * @param {EnginePlugin} plugin
 */
export function registerEngine(plugin) {
  if (!plugin || !plugin.id) throw new Error("Engine plugin must have an id");
  registry.set(plugin.id, plugin);
}

/**
 * Unregisters an engine plugin by id.
 * @param {string} id
 */
export function unregisterEngine(id) {
  registry.delete(id);
}

/**
 * Retrieves the Svelte component for an engine, falling back to GenericEngine if not found.
 * @param {string} [id]
 * @returns {any}
 */
export function getEngineComponent(id) {
  if (!id) return GenericEngine;
  return registry.get(id)?.component ?? GenericEngine;
}

/**
 * Lists all currently registered engine plugins.
 * @returns {EnginePlugin[]}
 */
export function listRegisteredEngines() {
  return Array.from(registry.values());
}
