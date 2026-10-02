// Shared by release.mjs. Turns monorepo commits that touched apps/reveal into
// Keep-a-Changelog style sections.

import { execFileSync } from "node:child_process";

export const REVEAL_DIR = "apps/reveal";

const SECTIONS = ["Added", "Changed", "Fixed", "Performance"];

// Commits that say nothing to someone reading the changelog.
const NOISE = [
  /^chore(\(.*\))?[:!]/i,
  /^docs?(\(.*\))?[:!]/i,
  /^test(\(.*\))?[:!]/i,
  /^ci(\(.*\))?[:!]/i,
  /^wip\b/i,
  /^merge\b/i,
  /^save uncommitted/i,
  /^(big|a big one|big one|hola|the guide and brick \d)$/i,
];

// Conventional scopes that belong to another project even when the commit
// also touched apps/reveal (shared styles, the manual, the garden site…).
const FOREIGN_SCOPES = /^(styles|themes|ui|stnd\.build|stnd\.gd|skills|turbo|framework|garden|launcher|core)$/;

function git(args) {
  return execFileSync("git", args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

/** Commits touching apps/reveal in `range` (e.g. "reveal-v0.46.0..HEAD"), oldest first. */
export function commitsIn(range) {
  const args = ["log", "--reverse", "--format=%H%x1f%ad%x1f%s", "--date=short"];
  if (range) args.push(range);
  args.push("--", REVEAL_DIR);
  return git(args)
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const [hash, date, subject] = line.split("\x1f");
      return { hash, date, subject };
    });
}

function classify(subject) {
  const conventional = subject.match(/^(\w+)(?:\(([^)]*)\))?!?:\s*(.+)$/);
  const versioned = subject.match(/^Reveal\s+\d+\.\d+\.\d+:\s*(.+)$/);
  const plain = subject.match(/^Reveal:\s*(.+)$/);

  let type;
  let scope = "";
  let text;
  if (conventional && /^(feat|fix|perf|refactor|polish|tweak|i18n|build|style)$/.test(conventional[1])) {
    [, type, scope = "", text] = conventional;
  } else if (versioned || plain) {
    text = (versioned || plain)[1];
    type = /\bfix(es|ed)?\b/i.test(text) ? "fix" : "feat";
  } else {
    // Unconventional: keep it, but file it under Changed rather than guess.
    type = "refactor";
    text = subject;
  }

  if (FOREIGN_SCOPES.test(scope)) return null;
  if (NOISE.some((re) => re.test(subject))) return null;

  const section = { feat: "Added", fix: "Fixed", perf: "Performance" }[type] ?? "Changed";
  return { section, text: text.charAt(0).toUpperCase() + text.slice(1) };
}

/** Group commits into { Added: [...], Changed: [...], … }, dropping noise. */
export function group(commits) {
  const out = Object.fromEntries(SECTIONS.map((s) => [s, []]));
  for (const { subject } of commits) {
    const entry = classify(subject);
    if (entry) out[entry.section].push(entry.text);
  }
  return out;
}

export function renderSection(version, date, commits) {
  const grouped = group(commits);
  const body = SECTIONS.filter((s) => grouped[s].length)
    .map((s) => `### ${s}\n\n${grouped[s].map((t) => `- ${t}`).join("\n")}`)
    .join("\n\n");
  return `## ${version} — ${date}\n\n${body || "_Internal changes only._"}\n`;
}
