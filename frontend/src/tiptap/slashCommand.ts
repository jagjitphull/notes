import { Extension } from "@tiptap/core";
import type { Editor, Range } from "@tiptap/core";
import { PluginKey } from "@tiptap/pm/state";
import Suggestion from "@tiptap/suggestion";
import { VueRenderer } from "@tiptap/vue-3";
import SlashCommandList from "../components/SlashCommandList.vue";
import type { IconName } from "../components/icons/icons";

const TRIGGER = "/";

export interface SlashCommandItem {
  id: string;
  label: string;
  icon: IconName;
  keywords?: string[];
  run: (editor: Editor, range: Range) => void;
}

export interface SlashCommandOptions {
  getItems: () => SlashCommandItem[];
}

// Builds the "/" menu's item list - pulled out of Editor.vue as a plain
// function (rather than inlined in its computed) so it can be unit tested
// directly against a real Tiptap editor instance without going through
// the Suggestion popup's DOM/keyboard plumbing, which jsdom can't
// reliably simulate. `t` is vue-i18n's translate function.
export function buildSlashCommandItems(t: (key: string) => string): SlashCommandItem[] {
  return [
    {
      id: "heading1",
      label: t("editor.toolbar.heading1"),
      icon: "heading1",
      keywords: ["h1", "title"],
      run: (e, range) => e.chain().focus().deleteRange(range).toggleHeading({ level: 1 }).run(),
    },
    {
      id: "heading2",
      label: t("editor.toolbar.heading2"),
      icon: "heading2",
      keywords: ["h2", "subtitle"],
      run: (e, range) => e.chain().focus().deleteRange(range).toggleHeading({ level: 2 }).run(),
    },
    {
      id: "heading3",
      label: t("editor.toolbar.heading3"),
      icon: "heading3",
      keywords: ["h3"],
      run: (e, range) => e.chain().focus().deleteRange(range).toggleHeading({ level: 3 }).run(),
    },
    {
      id: "checklist",
      label: t("editor.toolbar.checklist"),
      icon: "checklist",
      keywords: ["todo", "task"],
      run: (e, range) => e.chain().focus().deleteRange(range).toggleTaskList().run(),
    },
    {
      id: "bulletList",
      label: t("editor.toolbar.bulletList"),
      icon: "bulletList",
      keywords: ["ul", "bullets"],
      run: (e, range) => e.chain().focus().deleteRange(range).toggleBulletList().run(),
    },
    {
      id: "numberedList",
      label: t("editor.toolbar.numberedList"),
      icon: "numberedList",
      keywords: ["ol", "ordered"],
      run: (e, range) => e.chain().focus().deleteRange(range).toggleOrderedList().run(),
    },
    {
      id: "codeBlock",
      label: t("editor.toolbar.codeBlock"),
      icon: "code",
      keywords: ["code", "```"],
      run: (e, range) => e.chain().focus().deleteRange(range).toggleCodeBlock().run(),
    },
    {
      id: "quote",
      label: t("editor.toolbar.quote"),
      icon: "quote",
      keywords: ["blockquote", ">"],
      run: (e, range) => e.chain().focus().deleteRange(range).toggleBlockquote().run(),
    },
    {
      id: "divider",
      label: t("editor.toolbar.divider"),
      icon: "divider",
      keywords: ["hr", "rule", "separator", "---"],
      run: (e, range) => e.chain().focus().deleteRange(range).setHorizontalRule().run(),
    },
    {
      id: "insertTable",
      label: t("editor.toolbar.insertTable"),
      icon: "table",
      keywords: ["grid"],
      run: (e, range) =>
        e.chain().focus().deleteRange(range).insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run(),
    },
    {
      id: "insertMath",
      label: t("editor.toolbar.insertMath"),
      icon: "math",
      keywords: ["latex", "equation", "katex"],
      run: (e, range) => {
        e.chain().focus().deleteRange(range).run();
        const { from } = e.state.selection;
        e.chain().focus().insertContent("$$").setTextSelection(from + 1).run();
      },
    },
    {
      id: "insertMermaid",
      label: t("editor.toolbar.insertMermaid"),
      icon: "diagram",
      keywords: ["graph", "flowchart", "mermaid"],
      run: (e, range) =>
        e
          .chain()
          .focus()
          .deleteRange(range)
          .insertContent({
            type: "codeBlock",
            attrs: { language: "mermaid" },
            content: [{ type: "text", text: "graph TD\n    A --> B" }],
          })
          .run(),
    },
  ];
}

// "/" at the start of a line opens a filterable menu of block-level
// insertions (heading, list, table, ...), Notion-style - every item just
// deletes the "/query" text it replaces and then runs the same editor
// command its toolbar-button equivalent does (see slashCommandItems in
// Editor.vue). Not a Node like NoteLink/MathInline: nothing is inserted
// *as* a slash command, so there's no markdown round-trip to define here.
export const SlashCommand = Extension.create<SlashCommandOptions>({
  name: "slashCommand",

  addOptions() {
    return { getItems: () => [] };
  },

  addProseMirrorPlugins() {
    return [
      Suggestion({
        editor: this.editor,
        // Suggestion's default pluginKey is a single module-level constant
        // shared by every caller that doesn't pass its own - NoteLink
        // already registers a Suggestion plugin on this editor, so without
        // a distinct key here ProseMirror rejects this as a duplicate.
        pluginKey: new PluginKey("slashCommand"),
        char: TRIGGER,
        startOfLine: true,
        items: ({ query }) => {
          const q = query.toLowerCase();
          if (!q) return this.options.getItems().slice(0, 10);
          return this.options
            .getItems()
            .filter(
              (item) =>
                item.label.toLowerCase().includes(q) ||
                item.keywords?.some((k) => k.includes(q)),
            )
            .slice(0, 10);
        },
        command: ({ editor, range, props }) => {
          (props as SlashCommandItem).run(editor, range);
        },
        render: () => {
          let component: VueRenderer;
          let unmount: (() => void) | undefined;

          return {
            onStart: (props) => {
              component = new VueRenderer(SlashCommandList, {
                props: { items: props.items, command: props.command },
                editor: props.editor,
              });
              if (!component.element) return;
              unmount = props.mount(component.element as HTMLElement);
            },
            onUpdate(props) {
              component.updateProps({ items: props.items, command: props.command });
            },
            onKeyDown(props) {
              if (props.event.key === "Escape") {
                unmount?.();
                return true;
              }
              return component.ref?.onKeyDown(props) ?? false;
            },
            onExit() {
              unmount?.();
              component.destroy();
            },
          };
        },
      }),
    ];
  },
});
