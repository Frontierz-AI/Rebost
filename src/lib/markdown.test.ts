/** @vitest-environment jsdom */

import { describe, expect, it } from "vitest";
import { isSafeHref, renderMarkdown } from "./markdown";
import type { SourcePassage } from "./api";

const source: SourcePassage = {
  sid: "S1",
  documentId: "d1",
  shelfId: "s1",
  title: "Contract",
  path: "/tmp/contract.md",
  body: "ninety days",
  pageStart: 2,
  score: 1,
};

describe("renderMarkdown", () => {
  it("strips javascript: hrefs", () => {
    const html = renderMarkdown("[x](javascript:alert(1))");
    expect(html).not.toMatch(/javascript:/i);
    expect(html).not.toContain("href=");
  });

  it("adds rel=noopener noreferrer on http links", () => {
    const html = renderMarkdown("[docs](https://example.com/a)");
    expect(html).toContain('href="https://example.com/a"');
    expect(html).toContain('rel="noopener noreferrer"');
    expect(html).toContain('target="_blank"');
  });

  it("does not execute raw HTML", () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)"><script>alert(1)</script>');
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<script");
    expect(html).not.toContain("onerror");
  });

  it("only turns real citation ids into buttons", () => {
    const html = renderMarkdown("See [S1] and [S99].", [source]);
    expect(html).toContain('data-sid="S1"');
    expect(html).toContain('class="cite-chip"');
    expect(html).not.toContain('data-sid="S99"');
    expect(html).toContain("[S99]");
  });

  it("drops forged cite buttons in model HTML", () => {
    const html = renderMarkdown(
      '<button class="cite-chip" data-sid="S1">S1</button><button>steal</button>',
    );
    expect(html).not.toContain(">steal<");
  });
});

describe("isSafeHref", () => {
  it("allows http(s) and mailto only", () => {
    expect(isSafeHref("https://example.com")).toBe(true);
    expect(isSafeHref("mailto:a@b.c")).toBe(true);
    expect(isSafeHref("javascript:alert(1)")).toBe(false);
    expect(isSafeHref("data:text/html,x")).toBe(false);
  });
});
