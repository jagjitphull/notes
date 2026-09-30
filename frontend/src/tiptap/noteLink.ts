import { mergeAttributes, Node } from "@tiptap/core";
import Suggestion from "@tiptap/suggestion";
import { VueNodeViewRenderer, VueRenderer } from "@tiptap/vue-3";
import type MarkdownIt from "markdown-it";
import type StateInline from "markdown-it/lib/rules_inline/state_inline";
import NoteLinkSuggestionList from "../components/NoteLinkSuggestionList.vue";
import NoteLinkView from "../components/NoteLinkView.vue";

const TRIGGER = "[[";

// [[Note Title]] - a wikilink to another note, matched by title (not a
// stable id): renaming the target note leaves existing links pointing at
// the old title, which then render as "broken" until fixed by hand. No
// auto-rename-propagation in v1 - simplicity over a link-integrity system.
function noteLinkMarkdownItPlugin(md: MarkdownIt) {
  md.inline.ruler.before("link", "note_link", (state: StateInline, silent: boolean) => {
    const src = state.src;
    const start = state.pos;
    if (src.charCodeAt(start) !== 0x5b || src.charCodeAt(start + 1) !== 0x5b) return false;

    const end = src.indexOf("]]", start + 2);
    if (end === -1) return false;

    const title = src.slice(start + 2, end).trim();
    if (!title || title.includes("\n")) return false;

    if (!silent) {
      const token = state.push("note_link", "", 0);
      token.meta = { title };
    }
    state.pos = end + 2;
    return true;
  });

  md.renderer.rules.note_link = (tokens, idx) => {
    const title = tokens[idx].meta.title as string;
    const escaped = md.utils.escapeHtml(title);
    return `<span data-note-link data-title="${escaped}">${escaped}</span>`;
  };
}

export interface NoteLinkOptions {
  getNoteTitles: () => string[];
  isResolved: (title: string) => boolean;
  onNavigate: (title: string) => void;
}

// Pulls every [[Title]] out of a note's raw markdown/plaintext (not the
// live editor) - used to compute backlinks from `note.plaintextContent`,
// which already holds each note's full saved body without a per-note
// fetch. Matches the same literal syntax the markdown-it rule above
// parses, so a title found here is exactly what that rule would turn
// into a link on that note's own next parse.
export function extractLinkedTitles(text: string): string[] {
  const titles: string[] = [];
  for (const match of text.matchAll(/\[\[([^[\]]+)\]\]/g)) {
    const title = match[1].trim();
    if (title) titles.push(title);
  }
  return titles;
}

export const NoteLink = Node.create<NoteLinkOptions>({
  name: "noteLink",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  draggable: false,

  addOptions() {
    return {
      getNoteTitles: () => [],
      isResolved: () => false,
      onNavigate: () => {},
    };
  },

  addAttributes() {
    return {
      title: {
        default: "",
        parseHTML: (element) => element.getAttribute("data-title") ?? "",
        renderHTML: (attributes) => ({ "data-title": attributes.title }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-note-link]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "span",
      mergeAttributes(HTMLAttributes, { "data-note-link": "" }),
      (HTMLAttributes.title as string) ?? "",
    ];
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: { write: (text: string) => void }, node: { attrs: { title: string } }) {
          state.write(`[[${node.attrs.title}]]`);
        },
        parse: {
          setup(markdownit: MarkdownIt) {
            markdownit.use(noteLinkMarkdownItPlugin);
          },
        },
      },
    };
  },

  addNodeView() {
    return VueNodeViewRenderer(NoteLinkView);
  },

  // Deliberately no addInputRules() for live "]]"-closes-the-link
  // conversion: it fires while the Suggestion popup below is still open
  // and tracking the same [[ trigger, and the two stepping on the same
  // transaction corrupts the surrounding text. A manually-typed [[Title]]
  // still becomes a real link - just on next parse (reopening the note),
  // via the markdown-it rule in addStorage() below, rather than live.

  addProseMirrorPlugins() {
    return [
      Suggestion({
        editor: this.editor,
        char: TRIGGER,
        // Note titles are usually multiple words ("Gamma Note"); without
        // this the query - and the whole suggestion match - ends at the
        // first space, breaking anything past a single word.
        allowSpaces: true,
        items: ({ query }) => {
          const q = query.toLowerCase();
          return this.options
            .getNoteTitles()
            .filter((title) => title.toLowerCase().includes(q))
            .slice(0, 8);
        },
        command: ({ editor, range, props }) => {
          editor
            .chain()
            .focus()
            .insertContentAt(range, [
              { type: this.name, attrs: { title: props as string } },
              { type: "text", text: " " },
            ])
            .run();
        },
        render: () => {
          let component: VueRenderer;
          let unmount: (() => void) | undefined;

          return {
            onStart: (props) => {
              component = new VueRenderer(NoteLinkSuggestionList, {
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
