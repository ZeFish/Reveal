// Tauri has no Node.js server: adapter-static with an index.html fallback
// puts the site in SPA mode. See https://v2.tauri.app/start/frontend/sveltekit/
import adapter from "@sveltejs/adapter-static";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  kit: {
    adapter: adapter({
      fallback: "index.html",
    }),
    alias: {
      "@modules": "./modules",
    },
  },
};

export default config;
