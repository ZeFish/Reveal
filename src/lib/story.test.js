import { describe, it, expect } from "vitest";
import { parseStory, serializeStory, storyRows, moveBlock, moveBlockBefore, removeBlock, normalizeRows, settleRows } from "./story.js";

/** @param {string} stem @param {boolean} [rowBreak] @param {string} [text] */
const photo = (stem, rowBreak = true, text = "") => ({ id: stem, isPhoto: true, stem, text, rowBreak });
/** @param {string} id @param {string} text */
const prose = (id, text) => ({ id, isPhoto: false, stem: "", text, rowBreak: true });
/** @param {any[]} blocks */
const shape = (blocks) => storyRows(blocks).map((r) => (r.type === "photos" ? r.blocks.map((b) => b.stem).join("+") : `"${r.block.text}"`));

describe("reading the file", () => {
  it("reads embeds with no blank line between them as ONE row", () => {
    const { blocks } = parseStory("![[A.jpg]]\n![[B.jpg]]\n![[C.jpg]]\n\n![[D.jpg]]\n");
    expect(shape(blocks)).toEqual(["A+B+C", "D"]);
  });

  it("a blank line, a caption or a paragraph ends the row", () => {
    const { blocks } = parseStory("![[A.jpg]]\n> [!caption] hello\n![[B.jpg]]\n![[C.jpg]]\ntext\n![[D.jpg]]\n");
    expect(shape(blocks)).toEqual(["A", "B+C", '"text"', "D"]);
  });

  it("writes a grouped row back flush, and other rows apart", () => {
    const blocks = [photo("A"), photo("B", false), photo("C")];
    const out = serializeStory("", blocks);
    expect(out).toBe("![[A.jpg]]\n![[B.jpg]]\n\n![[C.jpg]]\n");
    expect(shape(parseStory(out).blocks)).toEqual(["A+B", "C"]);
  });

  it("writes a caption as the callout's body, not its title", () => {
    const out = serializeStory("", [photo("A", true, "Une pomme"), photo("B")]);
    expect(out).toBe("![[A.jpg]]\n> [!caption]\n> Une pomme\n\n![[B.jpg]]\n");
  });

  it("reads a caption from the title (older notes) or the body, and rewrites it as the body", () => {
    const legacy = parseStory("![[A.jpg]]\n> [!caption] Une pomme\n").blocks[0];
    const body = parseStory("![[A.jpg]]\n> [!caption]\n> Une pomme\n").blocks[0];
    expect(legacy.text).toBe("Une pomme");
    expect(body.text).toBe("Une pomme");
    expect(serializeStory("", [legacy])).toBe("![[A.jpg]]\n> [!caption]\n> Une pomme\n");
  });

  it("keeps the note the way toggle() writes it — one blank line apart — as separate rows", () => {
    const { blocks } = parseStory("![[A.jpg]]\n\n![[B.jpg]]\n");
    expect(shape(blocks)).toEqual(["A", "B"]);
  });
});

describe("arranging", () => {
  const three = () => [photo("A"), photo("B"), photo("C")];

  it("puts a photo beside another, to its right", () => {
    expect(shape(moveBlock(three(), "C", "A", "right"))).toEqual(["A+C", "B"]);
  });

  it("puts a photo beside another, to its left, taking the head of the row", () => {
    const out = moveBlock(three(), "C", "A", "left");
    expect(shape(out)).toEqual(["C+A", "B"]);
    expect(out[0].rowBreak).toBe(true);
  });

  it("gives a photo a row of its own above or below the target's whole row", () => {
    const grouped = [photo("A"), photo("B", false), photo("C")];
    expect(shape(moveBlock(grouped, "C", "B", "above"))).toEqual(["C", "A+B"]);
    expect(shape(moveBlock(grouped, "C", "A", "below"))).toEqual(["A+B", "C"]);
    expect(shape(moveBlock([photo("C"), photo("A"), photo("B", false)], "C", "B", "below"))).toEqual(["A+B", "C"]);
  });

  it("taking the first photo out of a row does not drop the rest into the row above", () => {
    const blocks = [photo("P"), photo("A"), photo("B", false), photo("C")];
    const out = moveBlock(blocks, "A", "C", "below");
    expect(shape(out)).toEqual(["P", "B", "C", "A"]);
    expect(shape(removeBlock(blocks, "A"))).toEqual(["P", "B", "C"]);
    expect(shape(moveBlockBefore(blocks, "A", null))).toEqual(["P", "B", "C", "A"]);
  });

  it("a photo grouped onto a captioned one joins the row, and the row keeps one caption", () => {
    const blocks = [photo("A", true, "under A"), photo("B")];
    const out = moveBlock(blocks, "B", "A", "right");
    expect(shape(out)).toEqual(["A+B"]);
    expect(out.map((b) => b.text)).toEqual(["", "under A"]); // on the last photo, where the callout follows
  });

  it("gathers the captions of a whole row onto its last photo", () => {
    const row = settleRows([photo("A", true, "one"), photo("B", false, "two"), photo("C", false)]);
    expect(shape(row)).toEqual(["A+B+C"]);
    expect(row.map((b) => b.text)).toEqual(["", "", "one two"]);
  });

  it("a caption typed on a photo of a row does not split the row", () => {
    const typed = [photo("A", true), photo("B", false, "note")];
    expect(shape(settleRows(typed))).toEqual(["A+B"]);
    expect(serializeStory("", settleRows(typed))).toBe("![[A.jpg]]\n![[B.jpg]]\n> [!caption]\n> note\n");
  });

  it("reads a caption under a row as the row's caption", () => {
    const { blocks } = parseStory("![[A.jpg]]\n![[B.jpg]]\n> [!caption]\n> under both\n");
    expect(shape(blocks)).toEqual(["A+B"]);
    expect(blocks.map((b) => b.text)).toEqual(["", "under both"]);
  });

  it("treats a paragraph as never sharing a row", () => {
    const blocks = [photo("A"), photo("B"), prose("t", "words")];
    expect(shape(moveBlock(blocks, "t", "A", "right"))).toEqual(["A", '"words"', "B"]);
  });

  it("normalizing never leaves a row the file could not express", () => {
    const out = normalizeRows([photo("A", false), prose("t", "x"), photo("B", false)]);
    expect(out.map((b) => b.rowBreak)).toEqual([true, true, true]);
  });
});
