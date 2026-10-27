import { describe, expect, it } from "vitest";
import { importFeedback, shelfCapMessage, waitingMessage } from "./shelf-cap";

describe("shelfCapMessage", () => {
  it("explains a full Shelf and points to the setting", () => {
    expect(shelfCapMessage(0)).toContain("1,000");
    expect(shelfCapMessage(0)).toContain("Files this Shelf reads");
  });

  it("mentions how many were added when some fit", () => {
    expect(shelfCapMessage(1)).toContain("1 file");
    expect(shelfCapMessage(40)).toContain("40 files");
    expect(shelfCapMessage(1000)).toContain("1,000 files");
  });

  it("uses the Shelf's own limit", () => {
    expect(shelfCapMessage(0, 5_000)).toContain("5,000");
    expect(shelfCapMessage(12, 2_500)).toContain("up to 2,500 files");
  });

  it("asks to remove files once the limit can't go higher", () => {
    expect(shelfCapMessage(0, 10_000).toLowerCase()).toContain("remove");
    expect(shelfCapMessage(0, 10_000)).not.toContain("Files this Shelf reads");
    expect(shelfCapMessage(3, 10_000)).not.toContain("Files this Shelf reads");
  });

  it("has no setting to point to for a conversation's files", () => {
    expect(shelfCapMessage(0, 1_000, false)).toContain("conversation");
    expect(shelfCapMessage(0, 1_000, false)).not.toContain("Files this Shelf reads");
    expect(shelfCapMessage(5, 1_000, false)).toContain("5 files");
  });
});

describe("importFeedback", () => {
  it("passes the Shelf's limit to the cap message", () => {
    expect(importFeedback(0, true, 0, 2_500)).toContain("2,500");
  });

  it("stays quiet when files were added under the cap", () => {
    expect(importFeedback(12, false)).toBeNull();
  });

  it("explains unsupported types when nothing was queued", () => {
    expect(importFeedback(0, false)).toContain("supported type");
  });

  it("explains a path that is too long", () => {
    expect(importFeedback(0, false, 1)).toContain("too long");
    expect(importFeedback(0, false, 1)).not.toContain("supported type");
    expect(importFeedback(2, false, 3)).toContain("2 files");
    expect(importFeedback(2, false, 3)).toContain("too long");
  });
});

describe("waitingMessage", () => {
  it("names one file and many files", () => {
    expect(waitingMessage(1)).toBe("1 file waiting");
    expect(waitingMessage(1488)).toBe("1,488 files waiting");
  });
});
