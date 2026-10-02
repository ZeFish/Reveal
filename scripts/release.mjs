#!/usr/bin/env node
// Cut a Reveal release.
//
//   node scripts/release.mjs <patch|minor|major|X.Y.Z> [--dry-run] [--tag]
//
// package.json is the single source of truth for the version. This script
// bumps it, mirrors the number into src-tauri/tauri.conf.json and the Cargo
// workspace, and prepends a changelog section built from the commits that
// touched apps/reveal since the previous `reveal-v*` tag.
//
// With --tag it also commits the release and creates the reveal-vX.Y.Z tag.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { commitsIn, renderSection } from "./lib/changelog.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const appDir = path.resolve(here, "..");
const file = (...p) => path.join(appDir, ...p);

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const tag = args.includes("--tag");
const bump = args.find((a) => !a.startsWith("--"));

if (!bump) {
  console.error("usage: release.mjs <patch|minor|major|X.Y.Z> [--dry-run] [--tag]");
  process.exit(1);
}

const pkg = JSON.parse(fs.readFileSync(file("package.json"), "utf8"));
const current = pkg.version;

function next(version, kind) {
  if (/^\d+\.\d+\.\d+/.test(kind)) return kind;
  const [major, minor, patch] = version.split(/[.-]/).map(Number);
  if (kind === "major") return `${major + 1}.0.0`;
  if (kind === "minor") return `${major}.${minor + 1}.0`;
  if (kind === "patch") return `${major}.${minor}.${patch + 1}`;
  throw new Error(`Unknown bump "${kind}"`);
}

const version = next(current, bump);
const lastTag = `reveal-v${current}`;

try {
  execFileSync("git", ["rev-parse", "--verify", `refs/tags/${lastTag}`], { stdio: "ignore" });
} catch {
  console.error(`No tag ${lastTag}: cannot tell what changed since ${current}.\nTag the commit that shipped it first: git tag ${lastTag} <commit>`);
  process.exit(1);
}

const commits = commitsIn(`${lastTag}..HEAD`);
const today = new Date().toISOString().slice(0, 10);
const section = renderSection(version, today, commits);

if (dryRun) {
  console.log(`${current} -> ${version}  (${commits.length} commits since ${lastTag})\n`);
  console.log(section);
  process.exit(0);
}

// package.json
pkg.version = version;
fs.writeFileSync(file("package.json"), JSON.stringify(pkg, null, 2) + "\n");

// tauri.conf.json — a targeted replace keeps the file's formatting intact.
const tauriPath = file("src-tauri", "tauri.conf.json");
fs.writeFileSync(tauriPath, fs.readFileSync(tauriPath, "utf8").replace(/("version"\s*:\s*")[^"]*(")/, `$1${version}$2`));

// Cargo workspace version, inherited by every crate through version.workspace.
const cargoPath = file("src-tauri", "Cargo.toml");
fs.writeFileSync(
  cargoPath,
  fs.readFileSync(cargoPath, "utf8").replace(/(\[workspace\.package\][^[]*?\nversion\s*=\s*")[^"]*(")/, `$1${version}$2`),
);

// Cargo.lock records the workspace crates' versions too; left alone, the next
// build rewrites it and leaves the tree dirty right after a release.
try {
  execFileSync("cargo", ["update", "--workspace", "--offline"], { cwd: file("src-tauri"), stdio: "ignore" });
} catch {
  console.warn("Could not refresh Cargo.lock (is cargo installed?). Run `cargo update --workspace` in src-tauri.");
}

// CHANGELOG.md — newest section goes right under the title.
const changelogPath = file("CHANGELOG.md");
const existing = fs.readFileSync(changelogPath, "utf8");
const [title, ...rest] = existing.split(/\n(?=## )/);
fs.writeFileSync(changelogPath, `${title.trimEnd()}\n\n${section}\n${rest.join("\n")}`);

console.log(`Reveal ${current} -> ${version}`);

if (tag) {
  const run = (...a) => execFileSync("git", a, { cwd: appDir, stdio: "inherit" });
  run("add", "package.json", "CHANGELOG.md", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock");
  run("commit", "-m", `chore(reveal): release v${version}`);
  run("tag", `reveal-v${version}`);
  console.log(`Committed and tagged reveal-v${version}. Push with: git push --follow-tags`);
} else {
  console.log("Review the diff, then commit and tag reveal-v" + version + " (or rerun with --tag).");
}
