import { mergeAttributes, Node } from "@tiptap/core";
import { VueNodeViewRenderer } from "@tiptap/vue-3";
import type MarkdownIt from "markdown-it";
import type StateInline from "markdown-it/lib/rules_inline/state_inline";
import FileAttachmentView from "../components/FileAttachmentView.vue";

// ![[path]] - an embedded non-image file (see Editor.vue's handleDrop/
// handlePaste), distinct from both a [[Title]] note link (no leading "!")
// and a standard markdown image (which needs "(url)", not "[[...]]").
// Matches Obsidian's own attachment-embed syntax. `path` is relative to
// notes_root, as returned by the save_attachment command - see
// store::save_attachment on the Rust side for where the file itself lives.
function fileAttachmentMarkdownItPlugin(md: MarkdownIt) {
  md.inline.ruler.before("image", "file_attachment", (state: StateInline, silent: boolean) => {
    const src = state.src;
    const start = state.pos;
    if (src.charCodeAt(start) !== 0x21) return false; // "!"
    if (src.charCodeAt(start + 1) !== 0x5b || src.charCodeAt(start + 2) !== 0x5b) return false; // "[["

    const end = src.indexOf("]]", start + 3);
    if (end === -1) return false;

    const path = src.slice(start + 3, end).trim();
    if (!path || path.includes("\n")) return false;

    if (!silent) {
      const token = state.push("file_attachment", "", 0);
      token.meta = { path };
    }
    state.pos = end + 2;
    return true;
  });

  md.renderer.rules.file_attachment = (tokens, idx) => {
    const path = tokens[idx].meta.path as string;
    const escaped = md.utils.escapeHtml(path);
    return `<span data-file-attachment data-path="${escaped}"></span>`;
  };
}

export const FileAttachment = Node.create({
  name: "fileAttachment",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      path: {
        default: "",
        parseHTML: (element) => element.getAttribute("data-path") ?? "",
        renderHTML: (attributes) => ({ "data-path": attributes.path }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-file-attachment]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes(HTMLAttributes, { "data-file-attachment": "" })];
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: { write: (text: string) => void }, node: { attrs: { path: string } }) {
          state.write(`![[${node.attrs.path}]]`);
        },
        parse: {
          setup(markdownit: MarkdownIt) {
            markdownit.use(fileAttachmentMarkdownItPlugin);
          },
        },
      },
    };
  },

  addNodeView() {
    return VueNodeViewRenderer(FileAttachmentView);
  },
});
