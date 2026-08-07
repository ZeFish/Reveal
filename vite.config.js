import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const host = process.env.TAURI_DEV_HOST;
const here = path.dirname(fileURLToPath(import.meta.url));

// What @stnd/core's Astro integration would normally inject as
// `virtual:stnd/config`. Reveal is not an Astro app, so the shim below
// serves this plain object instead — enough for @stnd/icon, @stnd/ui and
// @stnd/launcher to resolve their config at runtime.
const stndConfig = {
  name: "Reveal",
  icon: { library: "ph" },
};

// @stnd/icon fetches SVGs from /icons/{prefix}/{name}.svg; in the Astro apps
// the integration copies them into the build. Here the dev server proxies the
// monorepo's icon files directly. Build-time copying lands with the first
// real icon usage (M2).
const iconsRoot = path.resolve(here, "../../packages/icon/icons");

/**
 * @returns {import("vite").Plugin}
 */
function stndShim() {
  const VIRTUAL = "virtual:stnd/config";
  const RESOLVED = "\0" + VIRTUAL;
  return {
    name: "stnd-shim",
    resolveId(id) {
      if (id === VIRTUAL) return RESOLVED;
    },
    load(id) {
      if (id === RESOLVED) return `export default ${JSON.stringify(stndConfig)};`;
    },
    configureServer(server) {
      server.middlewares.use("/icons", (req, res, next) => {
        if (!req.url) return next();
        const rel = decodeURIComponent(req.url.split("?")[0]);
        const file = path.join(iconsRoot, rel);
        if (!file.startsWith(iconsRoot) || !file.endsWith(".svg") || !fs.existsSync(file)) {
          return next();
        }
        res.setHeader("Content-Type", "image/svg+xml");
        res.end(fs.readFileSync(file));
      });
    },
  };
}

export default defineConfig(async () => ({
  plugins: [sveltekit(), stndShim()],

  // Tauri-specific: keep rust errors visible, fixed port, ignore src-tauri.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    fs: {
      allow: [
        path.resolve(here, "../.."),
      ],
    },
    watch: {
      ignored: ["**/src-tauri/**", "**/Sources/**", "**/engine/**", "**/.build/**"],
    },
  },
}));
