// The story note's block model — a faithful JS port of the Swift `StoryNote`
// blocks()/setBlocks() logic (Sources/Reveal/StoryNote.swift). The note's body
// is an ordered sequence of blocks: photos (an `![[stem.jpg]]` embed with an
// optional `> [!caption]` callout beneath) and prose runs. Consecutive bare
// embeds — no blank line, no caption between — render as ONE justified row in
// the Garden; a caption or a blank line breaks the row.
//
// This lets the composer be WYSIWYG (edit the arranged photos directly) while
// the file on disk stays a plain, hand-editable Markdown story.

/** @typedef {{ id: string, isPhoto: boolean, stem: string, text: string, rowBreak: boolean }} Block */

let _idCounter = 0;
const nextId = () => `blk${_idCounter++}`;

/** The text inside the first `![[ … ]]` on a line, or null.
 * @param {string} line
 * @returns {string | null}
 */
function embedInner(line) {
  const m = line.match(/!\[\[([^\]]+)\]\]/);
  return m ? m[1] : null;
}

/** `2026/…/IMG_4012.reveal.jpg` → `IMG_4012` (basename, no extension). Tolerates
 *  both the web `<stem>.jpg` and the Swift `<stem>.reveal.jpg` embed forms.
 * @param {string} inner
 * @returns {string}
 */
export function stemOf(inner) {
  let s = inner;
  const slash = s.lastIndexOf("/");
  if (slash !== -1) s = s.slice(slash + 1);
  const dot = s.lastIndexOf(".");
  if (dot > 0) s = s.slice(0, dot);
  if (s.endsWith(".reveal")) s = s.slice(0, -".reveal".length);
  return s;
}

/**
 * Extract `garden-url` (or `garden_url` / `url`) from YAML frontmatter or note text.
 * @param {string} content
 * @returns {string | null}
 */
export function extractGardenUrl(content) {
  if (!content) return null;
  const match = content.match(/garden[-_]url:\s*["']?([^"'\r\n]+)["']?/i);
  return match ? match[1].trim() : null;
}

/**
 * Parse story markdown → { frontmatter, blocks }. `frontmatter` is the raw
 * fenced block (`---\n…\n---`) preserved verbatim, or "" when absent.
 * @param {string} content
 * @returns {{ frontmatter: string, blocks: Block[] }}
 */
export function parseStory(content) {
  const text = (content ?? "").replace(/\r\n/g, "\n");
  let frontmatter = "";
  let body = text;
  if (text.startsWith("---\n")) {
    const end = text.indexOf("\n---", 4);
    if (end !== -1) {
      frontmatter = text.slice(0, end + 4); // through the closing `---`
      body = text.slice(end + 4).replace(/^\n+/, "");
    }
  }

  /** @type {Block[]} */
  const blocks = [];
  /** @type {string[]} */
  let prose = [];
  let rowOpen = false; // the line above was a captionless embed → same row
  let lastWasBlank = true; // was the line before this an empty line?
  // A hidden `<!--grid-anchor:STEM-->` line just above a paragraph records
  // which grid photo it visually sits after — possibly one not embedded in
  // the story at all. Without it (older notes), Grid falls back to the last
  // *embedded* photo seen, which is all it ever had to anchor to before.
  let pendingAnchorStem = "";

  const flushProse = () => {
    const t = prose.join("\n").trim();
    if (t) blocks.push({ id: nextId(), isPhoto: false, stem: pendingAnchorStem, text: t, rowBreak: true });
    prose = [];
    pendingAnchorStem = "";
  };

  const lines = body.split("\n");
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      rowOpen = false;
      lastWasBlank = true;
      i++;
      continue;
    }

    const inner = embedInner(line);
    if (inner) {
      flushProse();
      let caption = "";
      let j = i + 1;
      while (j < lines.length && lines[j].trim().startsWith(">")) {
        let s = lines[j].replace(/^[>\s]+/, "");
        if (s.toLowerCase().startsWith("[!caption]")) {
          s = s.slice("[!caption]".length).trim();
        }
        // `> [!caption] text` (the text as the callout's title, how older
        // notes were written) and `> [!caption]` + `> text` (as its body) read
        // the same.
        if (s) caption += (caption ? " " : "") + s;
        j++;
      }
      // Flush embeds — no blank line, no caption between — are ONE row: that
      // adjacency is exactly how a grouped row is written to the file, so it
      // must be read back as one. (Reading every embed as its own row threw
      // the grouping away on reload, and the next save wrote a blank line
      // between them for good.) `toggle` in story.rs separates the photos it
      // adds with a blank line, so quick-collection marks stay separate rows.
      const rowBreak = !rowOpen || lastWasBlank;
      blocks.push({ id: nextId(), isPhoto: true, stem: stemOf(inner), text: caption, rowBreak });
      rowOpen = caption === ""; // a caption closes the flush row
      lastWasBlank = false;
      i = j;
    } else {
      const anchorMatch = prose.length === 0 ? line.trim().match(/^<!--\s*grid-anchor:\s*(\S+?)\s*-->$/) : null;
      if (anchorMatch) {
        pendingAnchorStem = anchorMatch[1];
        i++;
        continue;
      }
      rowOpen = false;
      lastWasBlank = false;
      prose.push(line);
      i++;
    }
  }
  flushProse();
  return { frontmatter, blocks };
}

/**
 * Serialize blocks back to the full note text, preserving frontmatter. The web
 * embeds `<stem>.jpg` (the name publish/export develops to), matching the
 * existing web stories.
 * @param {string} frontmatter raw fenced block, or ""
 * @param {Block[]} blocks
 * @returns {string}
 */
export function serializeStory(frontmatter, blocks) {
  let out = "";
  let rowOpen = false; // last emitted block was a captionless photo
  for (const b of blocks) {
    if (b.isPhoto) {
      const embed = `![[${b.stem}.jpg]]`;
      const cap = (b.text ?? "").trim();
      if (rowOpen && !b.rowBreak) {
        out += "\n" + embed; // join the row above (flush embeds)
      } else {
        if (out) out += "\n\n";
        out += embed; // a new row
      }
      if (cap) {
        // The caption is the callout's BODY, not its title: a title-less
        // callout, then the text in `> ` lines (what the daily note writes too).
        out += `\n> [!caption]\n${cap.split("\n").map((l) => `> ${l}`).join("\n")}`;
        rowOpen = false; // the caption closes the row
      } else {
        rowOpen = true;
      }
    } else {
      const t = (b.text ?? "").trim();
      if (!t) continue;
      if (out) out += "\n\n";
      if (b.stem) out += `<!--grid-anchor:${b.stem}-->\n`;
      out += t;
      rowOpen = false;
    }
  }
  const body = out ? out + "\n" : "";
  const fm = (frontmatter ?? "").trim();
  return fm ? `${fm}\n\n${body}` : body;
}

/**
 * @typedef {{ type: "photos", blocks: Block[] }} StoryPhotosRow
 * @typedef {{ type: "prose", block: Block }} StoryProseRow
 */

/**
 * Group blocks into render rows: a run of flush photos → one `photos` row;
 * each prose block → its own `prose` row.
 * @param {Block[]} blocks
 * @returns {Array<StoryPhotosRow | StoryProseRow>}
 */
export function storyRows(blocks) {
  /** @type {Array<StoryPhotosRow | StoryProseRow>} */
  const rows = [];
  for (const b of blocks) {
    const last = rows[rows.length - 1];
    if (b.isPhoto && !b.rowBreak && last && last.type === "photos") {
      last.blocks.push(b);
    } else if (b.isPhoto) {
      rows.push({ type: "photos", blocks: [b] });
    } else {
      rows.push({ type: "prose", block: b });
    }
  }
  return rows;
}

// ---- arranging ---------------------------------------------------------
// Pure edits on the block list. A photo shares the row above it only when
// `rowBreak` is false AND the block before it is a captionless photo (a
// caption, a paragraph or a blank line ends a row in the file), so every edit
// ends by re-normalizing: the flags can never describe a row the Markdown
// could not express.

/**
 * Force `rowBreak` wherever the block before cannot carry a row.
 * @param {Block[]} blocks
 * @returns {Block[]}
 */
export function normalizeRows(blocks) {
  return blocks.map((b, i) => {
    if (!b.isPhoto || b.rowBreak) return b;
    const prev = i > 0 ? blocks[i - 1] : null;
    const canJoin = !!prev && prev.isPhoto && (prev.text ?? "").trim() === "";
    return canJoin ? b : { ...b, rowBreak: true };
  });
}

/**
 * A row has ONE caption, under the row — that is all the Markdown can say: a
 * `> [!caption]` callout closes the paragraph of images above it, so it sits
 * below the whole row on the page, never under a single photo of it. Rows are
 * read from the `rowBreak` flags, every caption of a multi-photo row is gathered
 * onto its last photo (the one the callout follows in the file), and only then
 * is the row structure normalized — so giving a photo a caption, or grouping a
 * captioned photo, no longer splits the row.
 * @param {Block[]} blocks
 * @returns {Block[]}
 */
export function settleRows(blocks) {
  const list = blocks.map((b) => ({ ...b }));
  let i = 0;
  while (i < list.length) {
    if (!list[i].isPhoto) {
      i++;
      continue;
    }
    let j = i;
    while (j + 1 < list.length && list[j + 1].isPhoto && !list[j + 1].rowBreak) j++;
    if (j > i) {
      const captions = [];
      for (let k = i; k <= j; k++) {
        const t = (list[k].text ?? "").trim();
        if (t) captions.push(t);
        list[k].text = "";
      }
      list[j].text = captions.join(" ");
    }
    i = j + 1;
  }
  return normalizeRows(list);
}

/**
 * Take a block out of the list. If it headed a row, the next photo in that
 * row becomes the head — otherwise it would fall into the row above.
 * @param {Block[]} blocks
 * @param {string} id
 * @returns {{ rest: Block[], moved: Block | null, index: number }}
 */
function extract(blocks, id) {
  const index = blocks.findIndex((b) => b.id === id);
  if (index < 0) return { rest: blocks, moved: null, index };
  const list = blocks.map((b) => ({ ...b }));
  const [moved] = list.splice(index, 1);
  const next = list[index];
  if (moved.isPhoto && moved.rowBreak && next?.isPhoto && !next.rowBreak) next.rowBreak = true;
  return { rest: list, moved, index };
}

/**
 * @param {Block[]} blocks
 * @param {string} id
 * @returns {Block[]}
 */
export function removeBlock(blocks, id) {
  const { rest, moved } = extract(blocks, id);
  return moved ? settleRows(rest) : blocks;
}

/**
 * Move a block to stand on its own row before `beforeId` (a row's first
 * block), or to the very end when `beforeId` is null.
 * @param {Block[]} blocks
 * @param {string} id
 * @param {string | null} beforeId
 * @returns {Block[]}
 */
export function moveBlockBefore(blocks, id, beforeId) {
  if (id === beforeId) return blocks;
  const { rest, moved } = extract(blocks, id);
  if (!moved) return blocks;
  if (moved.isPhoto) moved.rowBreak = true;
  let at = beforeId ? rest.findIndex((b) => b.id === beforeId) : rest.length;
  if (at < 0) at = rest.length;
  rest.splice(at, 0, moved);
  return settleRows(rest);
}

/** @typedef {"left" | "right" | "above" | "below"} DropPlace */

/**
 * Move a block relative to a target block. `left`/`right` put a photo in the
 * target's row, beside it; `above`/`below` give it a row of its own before or
 * after the target's whole row. A paragraph is never placed in a row, so it
 * treats left/right as above/below.
 * @param {Block[]} blocks
 * @param {string} id
 * @param {string} targetId
 * @param {DropPlace} place
 * @returns {Block[]}
 */
export function moveBlock(blocks, id, targetId, place) {
  if (id === targetId) return blocks;
  const { rest, moved } = extract(blocks, id);
  if (!moved) return blocks;
  const t = rest.findIndex((b) => b.id === targetId);
  if (t < 0) return blocks;

  const inRow = moved.isPhoto && rest[t].isPhoto;
  const side = place === "left" || place === "right" ? (inRow ? place : place === "left" ? "above" : "below") : place;

  let rowStart = t;
  while (rowStart > 0 && rest[rowStart].isPhoto && !rest[rowStart].rowBreak) rowStart--;
  let rowEnd = t;
  while (rowEnd + 1 < rest.length && rest[rowEnd + 1].isPhoto && !rest[rowEnd + 1].rowBreak) rowEnd++;

  if (side === "left") {
    // Beside the target: take over its place at the head of the row if it had it.
    moved.rowBreak = rest[t].rowBreak;
    rest[t].rowBreak = false;
    rest.splice(t, 0, moved);
  } else if (side === "right") {
    moved.rowBreak = false;
    rest.splice(t + 1, 0, moved);
  } else if (side === "above") {
    if (moved.isPhoto) moved.rowBreak = true;
    rest.splice(rowStart, 0, moved);
  } else {
    if (moved.isPhoto) moved.rowBreak = true;
    rest.splice(rowEnd + 1, 0, moved);
  }
  return settleRows(rest);
}
