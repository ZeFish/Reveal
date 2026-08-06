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

  const flushProse = () => {
    const t = prose.join("\n").trim();
    if (t) blocks.push({ id: nextId(), isPhoto: false, stem: "", text: t, rowBreak: true });
    prose = [];
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
        caption += (caption ? " " : "") + s;
        j++;
      }
      const rowBreak = !rowOpen || lastWasBlank;
      blocks.push({ id: nextId(), isPhoto: true, stem: stemOf(inner), text: caption, rowBreak });
      rowOpen = caption === ""; // a caption closes the flush row
      lastWasBlank = false;
      i = j;
    } else {
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
        out += `\n> [!caption] ${cap}`;
        rowOpen = false; // the caption closes the row
      } else {
        rowOpen = true;
      }
    } else {
      const t = (b.text ?? "").trim();
      if (!t) continue;
      if (out) out += "\n\n";
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
