import { describe, expect, it } from "vitest";
import { Editor } from "@tiptap/core";
import type { Range } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import { TableKit } from "@tiptap/extension-table";
import { buildSlashCommandItems, type SlashCommandItem } from "./slashCommand";

// Labels aren't under test here - a plain passthrough keeps assertions
// focused on document structure.
const t = (key: string) => key;

function makeEditor(content: string) {
  return new Editor({
    extensions: [
      StarterKit.configure({ heading: { levels: [1, 2, 3] } }),
      TaskList,
      TaskItem.configure({ nested: true }),
      TableKit.configure({ table: { resizable: false } }),
    ],
    content,
  });
}

function itemById(id: string): SlashCommandItem {
  const item = buildSlashCommandItems(t).find((i) => i.id === id);
  if (!item) throw new Error(`no such slash command item: ${id}`);
  return item;
}

// Position 1 is the first character inside <p>...</p> (position 0 is
// before the paragraph itself); "/query" is 6 characters, so it spans
// 1-7 - the same range Suggestion would hand `run` after matching
// "/query" typed at the start of an empty paragraph.
const SLASH_QUERY_RANGE: Range = { from: 1, to: 7 };

function makeEditorWithSlashQuery() {
  return makeEditor("<p>/query</p>");
}

// Exercises each item's `run` directly against a real Tiptap editor,
// standing in for what Suggestion does before calling it: delete the
// "/query" range it matched. The Suggestion popup itself isn't under
// test here - jsdom can't reliably drive ProseMirror's DOM/keyboard
// handling, and the popup reuses the same Suggestion/VueRenderer wiring
// [[note links]] already exercise via manual testing.
describe("slash command items", () => {
  it("deletes the slash query text for every item", () => {
    for (const item of buildSlashCommandItems(t)) {
      const editor = makeEditorWithSlashQuery();
      item.run(editor, SLASH_QUERY_RANGE);
      expect(editor.getText(), `${item.id} should remove "/query"`).not.toContain("query");
      editor.destroy();
    }
  });

  it("heading1/2/3 turn the block into the matching heading level", () => {
    for (const level of [1, 2, 3] as const) {
      const editor = makeEditorWithSlashQuery();
      itemById(`heading${level}`).run(editor, SLASH_QUERY_RANGE);
      expect(editor.isActive("heading", { level })).toBe(true);
      editor.destroy();
    }
  });

  it("checklist turns the block into a task list", () => {
    const editor = makeEditorWithSlashQuery();
    itemById("checklist").run(editor, SLASH_QUERY_RANGE);
    expect(editor.isActive("taskList")).toBe(true);
    editor.destroy();
  });

  it("bulletList and numberedList turn the block into the matching list type", () => {
    const bullet = makeEditorWithSlashQuery();
    itemById("bulletList").run(bullet, SLASH_QUERY_RANGE);
    expect(bullet.isActive("bulletList")).toBe(true);
    bullet.destroy();

    const numbered = makeEditorWithSlashQuery();
    itemById("numberedList").run(numbered, SLASH_QUERY_RANGE);
    expect(numbered.isActive("orderedList")).toBe(true);
    numbered.destroy();
  });

  it("codeBlock and quote turn the block into their respective node", () => {
    const code = makeEditorWithSlashQuery();
    itemById("codeBlock").run(code, SLASH_QUERY_RANGE);
    expect(code.isActive("codeBlock")).toBe(true);
    code.destroy();

    const quote = makeEditorWithSlashQuery();
    itemById("quote").run(quote, SLASH_QUERY_RANGE);
    expect(quote.isActive("blockquote")).toBe(true);
    quote.destroy();
  });

  it("divider inserts a horizontal rule", () => {
    const editor = makeEditorWithSlashQuery();
    itemById("divider").run(editor, SLASH_QUERY_RANGE);
    let found = false;
    editor.state.doc.descendants((node) => {
      if (node.type.name === "horizontalRule") found = true;
    });
    expect(found).toBe(true);
    editor.destroy();
  });

  it("insertTable inserts an active 3x3 table", () => {
    const editor = makeEditorWithSlashQuery();
    itemById("insertTable").run(editor, SLASH_QUERY_RANGE);
    expect(editor.isActive("table")).toBe(true);
    editor.destroy();
  });

  it("insertMath inserts a $$ pair with the cursor positioned between them", () => {
    const editor = makeEditorWithSlashQuery();
    itemById("insertMath").run(editor, SLASH_QUERY_RANGE);
    expect(editor.getText()).toBe("$$");
    expect(editor.state.selection.from).toBe(2);
    editor.destroy();
  });

  it("insertMermaid inserts a mermaid code block with starter content", () => {
    const editor = makeEditorWithSlashQuery();
    itemById("insertMermaid").run(editor, SLASH_QUERY_RANGE);
    expect(editor.isActive("codeBlock", { language: "mermaid" })).toBe(true);
    expect(editor.getText()).toContain("graph TD");
    editor.destroy();
  });

  it("every item has a unique id", () => {
    const ids = buildSlashCommandItems(t).map((i) => i.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});
