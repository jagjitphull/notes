// tiptap-markdown's own type declarations predate Tiptap v3 and don't
// augment @tiptap/core's Storage interface, so editor.storage.markdown is
// untyped without this. The runtime API (Markdown.configure(...),
// editor.storage.markdown.getMarkdown()) is documented and works against
// the v3 peer dependency the package declares.
import "@tiptap/core";

declare module "@tiptap/core" {
  interface Storage {
    markdown: {
      getMarkdown(): string;
    };
  }
}
