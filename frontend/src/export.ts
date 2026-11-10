import katex from "katex";
import mermaid from "mermaid";
// Vite's `?raw` suffix imports the file's contents as a plain string -
// inlined into the exported page's own <style> so the math it renders
// still looks right when opened offline, away from this app.
import katexCss from "katex/dist/katex.min.css?raw";

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

// Tiptap's static getHTML() serializes every node via its schema's
// renderHTML - correct for plain content, but the math/diagram nodes only
// look right through their live Vue NodeView (see MathInlineView.vue,
// CodeBlockView.vue), which a standalone export doesn't have. This walks
// the same parsed HTML and replaces each one with the same KaTeX/Mermaid
// rendering those NodeViews use, so the export looks the way the note
// actually looks in the app rather than showing raw LaTeX/diagram source.
async function renderMathAndDiagrams(doc: Document): Promise<void> {
  doc.querySelectorAll<HTMLElement>("span[data-math-inline]").forEach((el) => {
    const latex = el.getAttribute("data-latex") ?? "";
    try {
      el.innerHTML = katex.renderToString(latex, { throwOnError: true, displayMode: false });
    } catch {
      el.textContent = `$${latex}$`;
    }
  });

  mermaid.initialize({ startOnLoad: false, theme: "default" });
  for (const code of Array.from(doc.querySelectorAll<HTMLElement>("pre > code.language-mermaid"))) {
    const pre = code.parentElement;
    if (!pre) continue;
    try {
      const id = `mermaid-export-${Math.random().toString(36).slice(2)}`;
      const { svg } = await mermaid.render(id, (code.textContent ?? "").trim() || "graph TD\nA");
      const wrapper = doc.createElement("div");
      wrapper.className = "export-diagram";
      wrapper.innerHTML = svg;
      pre.replaceWith(wrapper);
    } catch {
      // Leave the raw diagram source in place - still readable as text.
    }
  }

  doc.querySelectorAll<HTMLElement>("pre > code.language-math").forEach((code) => {
    const pre = code.parentElement;
    if (!pre) return;
    try {
      const html = katex.renderToString(code.textContent ?? "", {
        throwOnError: true,
        displayMode: true,
      });
      const wrapper = doc.createElement("div");
      wrapper.className = "export-math";
      wrapper.innerHTML = html;
      pre.replaceWith(wrapper);
    } catch {
      // Leave as a plain code block.
    }
  });

  // A file attachment embed renders its name/icon via its own NodeView
  // (fetched from disk); a static export has neither, so it falls back to
  // just naming the file.
  doc.querySelectorAll<HTMLElement>("span[data-file-attachment]").forEach((el) => {
    const path = el.getAttribute("data-path") ?? "";
    el.textContent = `\u{1F4CE} ${path.split("/").pop() || path}`;
  });
}

const PAGE_CSS = `
  :root { color-scheme: light; }
  body {
    margin: 0;
    padding: 40px 20px;
    background: #fff;
    color: #1a1a1a;
    font: 15px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
  }
  article {
    max-width: 680px;
    margin: 0 auto;
  }
  article > *:first-child::first-line { font-size: 1.6em; font-weight: 700; }
  h1 { font-size: 1.5em; margin: 1.2em 0 0.4em; }
  h2 { font-size: 1.25em; margin: 1.1em 0 0.4em; }
  h3 { font-size: 1.1em; margin: 1em 0 0.4em; }
  p { margin: 0.6em 0; }
  a { color: #0a84ff; }
  blockquote {
    margin: 0.8em 0;
    padding: 0.2em 1em;
    border-left: 3px solid #d0d0d0;
    color: #555;
  }
  pre {
    background: #f4f4f5;
    padding: 12px 14px;
    border-radius: 8px;
    overflow-x: auto;
    font: 13px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  table { border-collapse: collapse; width: 100%; margin: 0.8em 0; }
  th, td { border: 1px solid #d8d8d8; padding: 6px 10px; text-align: left; }
  th { background: #f4f4f5; }
  mark { background: #fff1a8; border-radius: 2px; }
  ul[data-type="taskList"] { list-style: none; padding-left: 0; }
  ul[data-type="taskList"] li { display: flex; align-items: baseline; gap: 6px; }
  ul[data-type="taskList"] input[type="checkbox"] { pointer-events: none; }
  .export-diagram, .export-math { margin: 1em 0; text-align: center; overflow-x: auto; }
  .export-diagram svg { max-width: 100%; }
  span[data-note-link] {
    color: #0a84ff;
    background: rgba(10, 132, 255, 0.1);
    border-radius: 4px;
    padding: 0 3px;
  }
`;

/// Builds a complete, offline-viewable HTML document from a note's live
/// Tiptap HTML (`editor.getHTML()`), suitable for writing straight to a
/// .html file or feeding to the print dialog for a PDF.
export async function buildExportDocument(title: string, rawHtml: string): Promise<string> {
  const doc = new DOMParser().parseFromString(rawHtml, "text/html");
  await renderMathAndDiagrams(doc);

  return `<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<title>${escapeHtml(title)}</title>
<style>
${katexCss}
${PAGE_CSS}
</style>
</head>
<body>
<article>
${doc.body.innerHTML}
</article>
</body>
</html>
`;
}
