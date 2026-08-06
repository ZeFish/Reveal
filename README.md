# Reveal

> A native photo catalogue and darkroom, built by someone who spent fifteen years in the software he was trying to replace.

Reveal is a desktop application for filing, culling and developing photographs. It runs on Rust and Tauri, and it manages my own working library — sixteen thousand images — every day.

**[reveal.photos](https://reveal.photos)**

## Why it exists

Lightroom is very good at a great many things, and slightly wrong at the few that matter most to me: it owns your files, it hides your archive behind a proprietary catalogue, and its develop controls are named after sliders rather than after the process they imitate.

Reveal takes the opposite position on all three. Your originals stay where you put them, in folders you can read without this application. The catalogue is a SQLite file. And the develop panel speaks the language of a darkroom — emulsion, paper stock, Y and M filtration, development time — because those words describe what is actually being simulated.

## What it does

**Catalogue.** Multi-root: every folder you add is its own catalogue, so a NAS mounted two different ways doesn't become two conflicting libraries. RAW decoding through [`rawler`](https://github.com/dnglab/dnglab), EXIF, SQLite index, no proprietary container.

**Cull.** A grid built for going through a shoot quickly, with AI-assisted selection when the volume justifies it.

**Develop.** Interchangeable engines, each one a vertical slice of Rust that declares its own control groups. The frontend renders whatever the engine declares — adding an engine means writing one Rust module, not touching the interface. Film simulation runs on the GPU through [`spektrafilm-rs`](https://github.com/turbasvin/spektrafilm-rs): emulsions, paper stocks, enlarger filtration, halation, grain.

**Publish.** Straight to a [Standard Garden](https://standard.garden) site, because the last step of photography is showing the work.

## Credit where it belongs

The film simulation mathematics are **not** mine. They come from [`spektrafilm-rs`](https://github.com/turbasvin/spektrafilm-rs), pinned at v0.1.2. Reveal depends on that engine rather than reimplementing it — the right call for a one-person project, and the reason the simulation is any good.

What is built here is the application around it: the catalogue, the indexer, the import pipeline, the GPU integration, the culling, the engine-slice architecture, and the interface.

## Status

Pre-1.0 and honest about it. It is my daily tool, which means it is reliable for the paths I walk every day and rougher elsewhere. macOS is the only platform currently built and tested.

The interface is in French; the code, comments and documentation are in English.

## Building

```bash
pnpm install
pnpm tauri dev
```

Requires Rust and the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/).

## License

GPL-3.0-only — see [LICENSE](./LICENSE).

---

Built by [Francis Fontaine](https://francisfontaine.com) · [github.com/ZeFish](https://github.com/ZeFish)
