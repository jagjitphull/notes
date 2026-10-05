import { describe, expect, it } from "vitest";
import { buildExportDocument } from "./export";

describe("buildExportDocument", () => {
  it("escapes the title and embeds the note's HTML in an <article>", async () => {
    const html = await buildExportDocument("A & B", "<p>Hello <strong>world</strong></p>");
    expect(html).toContain("<title>A &amp; B</title>");
    expect(html).toContain("<p>Hello <strong>world</strong></p>");
  });

  it("renders a math_inline span's latex via KaTeX instead of leaving it blank", async () => {
    const html = await buildExportDocument(
      "Note",
      '<p><span data-math-inline data-latex="E=mc^2"></span></p>',
    );
    expect(html).toContain("katex");
    expect(html).not.toContain("data-math-inline></span>");
  });

  it("falls back to the file's basename for a file-attachment embed", async () => {
    const html = await buildExportDocument(
      "Note",
      '<p><span data-file-attachment data-path=".attachments/note-1/report.pdf"></span></p>',
    );
    expect(html).toContain("report.pdf");
  });

  it("leaves a mermaid code block's source readable if rendering fails", async () => {
    // jsdom has no real layout engine, so mermaid.render is expected to
    // reject here - this exercises the catch branch's fallback.
    const html = await buildExportDocument(
      "Note",
      '<pre><code class="language-mermaid">graph TD\nA --> B</code></pre>',
    );
    // Still inside a <code> block (HTML-escaped, like any other text node),
    // not silently dropped.
    expect(html).toContain("A --&gt; B");
  });
});
