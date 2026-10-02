// Reveal's face on the shared changelog reader (scripts/lib/changelog.mjs at
// the monorepo root): this directory, and the scopes that belong to other
// projects even when a commit also touched apps/reveal (shared styles, the
// manual, the garden site…).

import { createChangelog } from "../../../../scripts/lib/changelog.mjs";

export const REVEAL_DIR = "apps/reveal";

const changelog = createChangelog({
  dir: REVEAL_DIR,
  foreignScopes: /^(styles|themes|ui|stnd\.build|stnd\.gd|skills|turbo|framework|garden|launcher|core)$/,
});

export const { commitsIn, renderSection } = changelog;
