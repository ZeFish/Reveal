#!/usr/bin/env node
// Build the signed app and publish it as a GitHub Release on ZeFish/Reveal.
//
//   node scripts/publish.mjs [--dry-run]
//
// Run it after `release.mjs` has bumped, committed and tagged the version.
// What it does, in order:
//   1. builds the .app + .dmg and the updater bundle (Reveal.app.tar.gz + .sig),
//      signed with the updater key at ~/.tauri/reveal.key
//   2. writes latest.json — the feed the installed apps poll — with this
//      version's changelog section as the release notes
//   3. creates the release vX.Y.Z and uploads the four files
//
// --dry-run does 1 and 2 and stops, so nothing public happens.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "ZeFish/Reveal";
const KEY = path.join(os.homedir(), ".tauri", "reveal.key");

const appDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bundle = path.join(appDir, "src-tauri", "target", "release", "bundle");
const dryRun = process.argv.includes("--dry-run");

const { version } = JSON.parse(fs.readFileSync(path.join(appDir, "package.json"), "utf8"));
const tag = `v${version}`;

if (!fs.existsSync(KEY)) {
  console.error(`No updater signing key at ${KEY}.`);
  process.exit(1);
}

// 1. Build. The key goes in through the environment (the CLI wants its
// contents, not a path), never printed or put on a command line.
console.log(`Building Reveal ${version}…`);
execFileSync("pnpm", ["tauri", "build"], {
  cwd: appDir,
  stdio: "inherit",
  env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY: fs.readFileSync(KEY, "utf8"), TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "" },
});

const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
const tarball = path.join(bundle, "macos", "Reveal.app.tar.gz");
const signature = `${tarball}.sig`;
const dmg = path.join(bundle, "dmg", `Reveal_${version}_${arch}.dmg`);
for (const f of [tarball, signature, dmg]) {
  if (!fs.existsSync(f)) {
    console.error(`Missing build output: ${f}`);
    process.exit(1);
  }
}

// 2. latest.json. Notes are this version's section of CHANGELOG.md.
const changelog = fs.readFileSync(path.join(appDir, "CHANGELOG.md"), "utf8");
const section = changelog.split(/\n(?=## )/).find((s) => s.startsWith(`## ${version} `));
if (!section) {
  console.error(`CHANGELOG.md has no section for ${version}. Run release.mjs first.`);
  process.exit(1);
}
const notes = section.trim();

const feed = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    [`darwin-${arch}`]: {
      signature: fs.readFileSync(signature, "utf8").trim(),
      url: `https://github.com/${REPO}/releases/download/${tag}/Reveal.app.tar.gz`,
    },
  },
};
const out = path.join(bundle, "latest.json");
fs.writeFileSync(out, JSON.stringify(feed, null, 2) + "\n");
console.log(`Wrote ${out}`);

if (dryRun) {
  console.log("Dry run — nothing published.");
  process.exit(0);
}

// 3. Publish. The release lives on the public mirror, not the private monorepo.
const notesFile = path.join(bundle, "release-notes.md");
fs.writeFileSync(
  notesFile,
  `${notes.replace(/^## .*\n+/, "")}\n\n---\nReveal is not notarized by Apple. On first install, right-click the app and choose Open. Updates inside the app are signed and install without this step.\n`,
);
// The same disk image under a name without a version in it, so the website's
// `releases/latest/download/Reveal.dmg` link never has to change.
const stableDmg = path.join(bundle, "Reveal.dmg");
fs.copyFileSync(dmg, stableDmg);
execFileSync("gh", ["release", "create", tag, "--repo", REPO, "--title", `Reveal ${version}`, "--notes-file", notesFile, dmg, stableDmg, tarball, signature, out], {
  stdio: "inherit",
});
console.log(`Published https://github.com/${REPO}/releases/tag/${tag}`);
