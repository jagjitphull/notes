import { mergeAttributes, Node } from "@tiptap/core";
import { VueNodeViewRenderer } from "@tiptap/vue-3";
import type MarkdownIt from "markdown-it";
import type StateInline from "markdown-it/lib/rules_inline/state_inline";
import MathInlineView from "../components/MathInlineView.vue";

// $...$ - inline LaTeX math (rendered via KaTeX), e.g. "$E = mc^2$". A "$"
// immediately followed by a space or digit (or whose would-be closer is
// immediately preceded by a space) is left as plain text - notes that
// mention money ("$5", "costs $10 or $20") shouldn't have every dollar
// sign hijacked into a math span.
function mathInlineMarkdownItPlugin(md: MarkdownIt) {
  md.inline.ruler.before("escape", "math_inline", (state: StateInline, silent: boolean) => {
    const src = state.src;
    const start = state.pos;
    if (src.charCodeAt(start) !== 0x24) return false; // "$"

    const next = src.charCodeAt(start + 1);
    if (next === 0x20 || (next >= 0x30 && next <= 0x39)) return false;

    const end = src.indexOf("$", start + 1);
    if (end === -1) return false;
    if (src.charCodeAt(end - 1) === 0x20) return false;

    const latex = src.slice(start + 1, end);
    if (!latex || latex.includes("\n")) return false;

    if (!silent) {
      const token = state.push("math_inline", "", 0);
      token.meta = { latex };
    }
    state.pos = end + 1;
    return true;
  });

  md.renderer.rules.math_inline = (tokens, idx) => {
    const latex = tokens[idx].meta.latex as string;
    const escaped = md.utils.escapeHtml(latex);
    return `<span data-math-inline data-latex="${escaped}"></span>`;
  };
}

export const MathInline = Node.create({
  name: "mathInline",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      latex: {
        default: "",
        parseHTML: (element) => element.getAttribute("data-latex") ?? "",
        renderHTML: (attributes) => ({ "data-latex": attributes.latex }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-math-inline]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes(HTMLAttributes, { "data-math-inline": "" })];
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: { write: (text: string) => void }, node: { attrs: { latex: string } }) {
          state.write(`$${node.attrs.latex}$`);
        },
        parse: {
          setup(markdownit: MarkdownIt) {
            markdownit.use(mathInlineMarkdownItPlugin);
          },
        },
      },
    };
  },

  addNodeView() {
    return VueNodeViewRenderer(MathInlineView);
  },
});
