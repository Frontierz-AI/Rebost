import { describe, expect, it } from "vitest";
import type { ModelSearchResult } from "$lib/api";
import {
  EXPLORE_PAGE_SIZE,
  chipSortActive,
  columnAriaSort,
  isRecentRelease,
  modelBudgetBytes,
  nextExploreSort,
  normalizeExploreQuery,
  parseExploreRepoQuery,
  sortExploreResults,
  visibleExploreCount,
} from "./explore-models";

function hit(name: string, extras: Partial<ModelSearchResult> = {}): ModelSearchResult {
  return {
    id: name,
    name,
    source: "huggingface",
    reference: `org/${name}`,
    ...extras,
  };
}

const now = Date.parse("2026-08-20T12:00:00Z");
const gib = 1024 * 1024 * 1024;
const budget = 20 * gib;

describe("explore repo paste", () => {
  it("reads owner/repo and Hugging Face page URLs", () => {
    expect(parseExploreRepoQuery("OBLITERATUS/Qwen3.8-27B-OBLITERATED")).toBe(
      "OBLITERATUS/Qwen3.8-27B-OBLITERATED",
    );
    expect(
      parseExploreRepoQuery(
        " https://huggingface.co/OBLITERATUS/Qwen3.8-27B-OBLITERATED/tree/main ",
      ),
    ).toBe("OBLITERATUS/Qwen3.8-27B-OBLITERATED");
    expect(parseExploreRepoQuery("hf.co/unsloth/Qwen3-0.6B-GGUF")).toBe("unsloth/Qwen3-0.6B-GGUF");
    expect(parseExploreRepoQuery("<https://huggingface.co/Qwen/Qwen3-8B>")).toBe("Qwen/Qwen3-8B");
    expect(normalizeExploreQuery("  https://huggingface.co/Qwen/Qwen3-8B  ")).toBe("Qwen/Qwen3-8B");
    expect(parseExploreRepoQuery("Qwen3")).toBeUndefined();
    expect(parseExploreRepoQuery("https://evil.example/owner/repo")).toBeUndefined();
    expect(parseExploreRepoQuery("https://huggingface.co/datasets/owner/repo")).toBeUndefined();
    expect(parseExploreRepoQuery("owner/repo/extra")).toBeUndefined();
  });
});

describe("explore sort", () => {
  it("prefers official recent AIs that use this computer well", () => {
    const sorted = sortExploreResults(
      [
        hit("too-large", {
          released: "2026-08-01",
          downloads: 42_000_000,
          fits: false,
          sizeBytes: 40 * gib,
        }),
        hit("official-old-rightsize", {
          official: true,
          released: "2025-01-01",
          downloads: 8_000_000,
          fits: true,
          sizeBytes: 9 * gib,
        }),
        hit("unofficial-recent-tiny", {
          released: "2026-08-10",
          downloads: 50,
          fits: true,
          sizeBytes: 400 * 1024 * 1024,
        }),
        hit("official-recent-rightsize", {
          official: true,
          released: "2026-08-01",
          downloads: 120_000,
          fits: true,
          sizeBytes: 9 * gib,
        }),
        hit("undated", { downloads: 1 }),
      ],
      "best",
      now,
      budget,
    ).map((r) => r.name);
    expect(sorted[0]).toBe("official-recent-rightsize");
    expect(sorted[1]).toBe("official-old-rightsize");
    expect(sorted.at(-1)).toBe("too-large");
    expect(sorted.indexOf("unofficial-recent-tiny")).toBeLessThan(sorted.indexOf("too-large"));
  });

  it("orders by newest, smallest file, and most downloads", () => {
    const rows = [
      hit("old-big", { released: "2025-01-01", sizeBytes: 8, downloads: 10 }),
      hit("new-small", { released: "2026-08-01", sizeBytes: 2, downloads: 3 }),
    ];
    expect(sortExploreResults(rows, "released", now).map((r) => r.name)).toEqual([
      "new-small",
      "old-big",
    ]);
    expect(sortExploreResults(rows, "size", now).map((r) => r.name)).toEqual([
      "new-small",
      "old-big",
    ]);
    expect(sortExploreResults(rows, "downloads", now).map((r) => r.name)).toEqual([
      "old-big",
      "new-small",
    ]);
    expect(sortExploreResults(rows, "released", now, undefined, "asc").map((r) => r.name)).toEqual([
      "old-big",
      "new-small",
    ]);
    expect(sortExploreResults(rows, "size", now, undefined, "desc").map((r) => r.name)).toEqual([
      "old-big",
      "new-small",
    ]);
    expect(sortExploreResults(rows, "downloads", now, undefined, "asc").map((r) => r.name)).toEqual(
      ["new-small", "old-big"],
    );
  });

  it("toggles a column header between default and reverse", () => {
    expect(nextExploreSort("best", "desc", "released")).toEqual({ sort: "released", dir: "desc" });
    expect(nextExploreSort("released", "desc", "released")).toEqual({
      sort: "released",
      dir: "asc",
    });
    expect(nextExploreSort("released", "asc", "released")).toEqual({
      sort: "released",
      dir: "desc",
    });
    expect(nextExploreSort("released", "asc", "size")).toEqual({ sort: "size", dir: "asc" });
    expect(chipSortActive("released", "released", "desc")).toBe(true);
    expect(chipSortActive("released", "released", "asc")).toBe(false);
  });

  it("treats the last 90 days as recent", () => {
    expect(isRecentRelease("2026-05-22", now)).toBe(true);
    expect(isRecentRelease("2026-05-21", now)).toBe(false);
    expect(isRecentRelease(undefined, now)).toBe(false);
  });

  it("pages at 50 and never past the end", () => {
    expect(EXPLORE_PAGE_SIZE).toBe(50);
    expect(visibleExploreCount(75, 1)).toBe(50);
    expect(visibleExploreCount(75, 2)).toBe(75);
    expect(visibleExploreCount(12, 1)).toBe(12);
  });

  it("sizes the budget the same way as the catalog", () => {
    expect(modelBudgetBytes(32 * gib)).toBe(Math.floor(32 * gib * 0.65));
    expect(columnAriaSort("size", "size")).toBe("ascending");
    expect(columnAriaSort("size", "size", "desc")).toBe("descending");
    expect(columnAriaSort("downloads", "downloads")).toBe("descending");
    expect(columnAriaSort("released", "best")).toBe("none");
  });
});
