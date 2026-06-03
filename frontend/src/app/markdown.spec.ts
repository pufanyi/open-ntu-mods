import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders safe markdown and strips obvious XSS vectors", () => {
    const html = renderMarkdown(
      "# Title\n\n[ok](https://example.com)\n\n[bad](javascript:alert(1))\n\n<script>alert(1)</script>",
    );

    expect(html).toContain("<h1>Title</h1>");
    expect(html).toContain('href="https://example.com"');
    expect(html).not.toContain("javascript:");
    expect(html).not.toContain("<script>");
  });
});
