import { Extension, type Editor } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

export interface FindMatch {
  from: number;
  to: number;
}

const findReplacePluginKey = new PluginKey<DecorationSet>("findReplace");

// Decorations are the only state this extension owns (which positions to
// highlight, and how) - the search itself, the query text, and the
// "current match" index all live in Editor.vue as plain reactive state.
// This keeps the extension a thin rendering layer: Editor.vue computes
// fresh matches and hands them over via highlightMatches(); a doc edit
// with no fresh matches supplied just remaps the existing highlights
// through the change so they don't end up pointing at the wrong text.
export const FindReplace = Extension.create({
  name: "findReplace",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: findReplacePluginKey,
        state: {
          init: () => DecorationSet.empty,
          apply(tr, old) {
            const next = tr.getMeta(findReplacePluginKey) as DecorationSet | undefined;
            if (next) return next;
            return tr.docChanged ? old.map(tr.mapping, tr.doc) : old;
          },
        },
        props: {
          decorations(state) {
            return findReplacePluginKey.getState(state);
          },
        },
      }),
    ];
  },
});

export function findMatches(editor: Editor, query: string, caseSensitive: boolean): FindMatch[] {
  if (!query) return [];
  const matches: FindMatch[] = [];
  const needle = caseSensitive ? query : query.toLowerCase();
  editor.state.doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return;
    const haystack = caseSensitive ? node.text : node.text.toLowerCase();
    let from = 0;
    for (;;) {
      const index = haystack.indexOf(needle, from);
      if (index === -1) break;
      matches.push({ from: pos + index, to: pos + index + query.length });
      from = index + needle.length;
    }
  });
  return matches;
}

export function highlightMatches(editor: Editor, matches: FindMatch[], currentIndex: number) {
  const decorations = matches.map((match, i) =>
    Decoration.inline(match.from, match.to, {
      class: i === currentIndex ? "find-match find-match-current" : "find-match",
    }),
  );
  editor.view.dispatch(
    editor.state.tr.setMeta(findReplacePluginKey, DecorationSet.create(editor.state.doc, decorations)),
  );
}

export function clearHighlights(editor: Editor) {
  editor.view.dispatch(editor.state.tr.setMeta(findReplacePluginKey, DecorationSet.empty));
}

export function scrollToMatch(editor: Editor, match: FindMatch) {
  const node = editor.view.domAtPos(match.from).node;
  const el = node instanceof HTMLElement ? node : node.parentElement;
  el?.scrollIntoView({ block: "center", behavior: "smooth" });
}

// Replaces via a raw transaction (not editor.chain()) so this never moves
// DOM focus into the editor - the user stays in the find bar, free to
// keep hitting Replace/Next without focus bouncing away each time.
export function replaceMatch(editor: Editor, match: FindMatch, replacement: string) {
  editor.view.dispatch(editor.state.tr.insertText(replacement, match.from, match.to));
}

export function replaceAllMatches(editor: Editor, matches: FindMatch[], replacement: string) {
  if (matches.length === 0) return;
  let tr = editor.state.tr;
  // Right-to-left so replacing one match never invalidates the still-
  // pending positions of the ones before it in the same transaction.
  for (let i = matches.length - 1; i >= 0; i--) {
    tr = tr.insertText(replacement, matches[i].from, matches[i].to);
  }
  editor.view.dispatch(tr);
}
