import { marked } from "marked";
import createDOMPurify from "dompurify";
import type { SourcePassage } from "$lib/api";

const ALLOWED_TAGS = [
  "p",
  "br",
  "strong",
  "em",
  "a",
  "ul",
  "ol",
  "li",
  "code",
  "pre",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "blockquote",
  "hr",
  "table",
  "thead",
  "tbody",
  "tr",
  "th",
  "td",
  "button",
  "span",
];

const ALLOWED_ATTR = ["href", "class", "title", "data-sid", "rel", "target", "type"];

let purify: ReturnType<typeof createDOMPurify> | null = null;

function getPurify(): ReturnType<typeof createDOMPurify> {
  if (typeof window === "undefined") {
    throw new Error("Markdown sanitization needs a DOM");
  }
  if (!purify) {
    purify = createDOMPurify(window);
    purify.addHook("afterSanitizeAttributes", (node) => {
      if (!(node instanceof Element)) return;
      if (node.tagName === "A") {
        const href = node.getAttribute("href") ?? "";
        if (!isSafeHref(href)) {
          node.removeAttribute("href");
        }
        node.setAttribute("rel", "noopener noreferrer");
        node.setAttribute("target", "_blank");
      }
      if (node.tagName === "BUTTON") {
        if (node.getAttribute("class") !== "cite-chip") {
          node.remove();
        } else {
          node.setAttribute("type", "button");
        }
      }
    });
  }
  return purify;
}

export function isSafeHref(href: string): boolean {
  const trimmed = href.trim().toLowerCase();
  return (
    trimmed.startsWith("https:") || trimmed.startsWith("http:") || trimmed.startsWith("mailto:")
  );
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

/** Reopen ATX headings that ingest flattened onto one line. */
export function formatCardSummary(summary: string, outlineTitles: string[] = []): string {
  const reopened = summary.replace(/[^\S\n]+(?=#{1,6} )/g, "\n\n").trim();
  const titles = outlineTitles
    .map((title) => title.trim())
    .filter((title) => title.length > 0)
    .sort((a, b) => b.length - a.length);
  if (titles.length === 0) return reopened;

  return reopened
    .split("\n")
    .map((line) => {
      const heading = /^(#{1,6}) (.+)$/.exec(line);
      const hashes = heading?.[1];
      const rest = heading?.[2];
      if (!hashes || !rest) return line;
      const title = titles.find(
        (candidate) => rest === candidate || rest.startsWith(`${candidate} `),
      );
      if (!title || rest === title) return line;
      const after = rest.slice(title.length).trim();
      if (!after) return line;
      return `${hashes} ${title}\n\n${after}`;
    })
    .join("\n");
}

/** Markdown → sanitized HTML. Citation chips are only emitted for provided ids. */
export function renderMarkdown(text: string, sources: SourcePassage[] = []): string {
  let rendered = marked.parse(text, { async: false, breaks: true }) as string;
  if (sources.length > 0) {
    rendered = rendered.replace(/\[(S\d+)\]/g, (match, sid: string) => {
      const source = sources.find((s) => s.sid === sid);
      if (!source) return match;
      const title = escapeHtml(
        `${source.title}${source.pageStart ? ` · p. ${source.pageStart}` : ""}`,
      );
      return `<button type="button" class="cite-chip" data-sid="${sid}" title="${title}">${sid}</button>`;
    });
  }
  return getPurify().sanitize(rendered, {
    ALLOWED_TAGS,
    ALLOWED_ATTR,
    ALLOW_DATA_ATTR: true,
    ALLOWED_URI_REGEXP: /^(?:(?:https?|mailto):)/i,
    FORBID_TAGS: ["script", "iframe", "object", "embed", "form", "img", "svg", "math"],
  });
}
