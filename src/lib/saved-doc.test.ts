import { describe, expect, it } from "vitest";
import type { SavedDocument, StoredMessage } from "./api";
import { canSaveToShelf, savedNotice, SAVE_FORMATS, SHELF_FULL, withSaved } from "./saved-doc";

function answer(over: Partial<StoredMessage> = {}): StoredMessage {
  return {
    id: "m_1",
    role: "assistant",
    text: "# Plan\n\nStep one.",
    ts: "2026-11-25T10:00:00Z",
    sources: [],
    status: "done",
    ...over,
  };
}

const saved: SavedDocument = {
  status: "saved",
  fileName: "Plan.docx",
  format: "docx",
  shelfId: "s_1",
  documentId: "d_1",
  path: "/Shelves/Docs/Plan.docx",
};

describe("canSaveToShelf", () => {
  it("offers the button on a finished answer with a Shelf", () => {
    expect(canSaveToShelf(answer(), "s_1")).toBe(true);
  });

  it("hides it without a Shelf, while busy, or for unfinished, empty, or user messages", () => {
    expect(canSaveToShelf(answer(), null)).toBe(false);
    expect(canSaveToShelf(answer(), "s_1", true)).toBe(false);
    expect(canSaveToShelf(answer({ status: "stopped" }), "s_1")).toBe(false);
    expect(canSaveToShelf(answer({ text: "  " }), "s_1")).toBe(false);
    expect(canSaveToShelf(answer({ role: "user" }), "s_1")).toBe(false);
  });

  it("hides it once saved, and offers it again after Undo or a failure", () => {
    expect(canSaveToShelf(answer({ saved }), "s_1")).toBe(false);
    expect(canSaveToShelf(answer({ saved: { ...saved, status: "undone" } }), "s_1")).toBe(true);
    expect(
      canSaveToShelf(answer({ saved: { ...saved, status: "failed", error: SHELF_FULL } }), "s_1"),
    ).toBe(true);
  });
});

describe("savedNotice", () => {
  it("names the file and allows Open and Undo after a save", () => {
    expect(savedNotice(saved)).toEqual({
      key: "chat.savedTo",
      params: { file: "Plan.docx" },
      tone: "ok",
      canOpen: true,
      canUndo: true,
    });
  });

  it("explains a full Shelf and other failures", () => {
    expect(savedNotice({ ...saved, status: "failed", error: SHELF_FULL })?.key).toBe(
      "chat.saveShelfFull",
    );
    const other = savedNotice({ ...saved, status: "failed", error: "Disk is full." });
    expect(other?.key).toBe("chat.saveFailedNamed");
    expect(other?.params.reason).toBe("Disk is full.");
    expect(other?.canUndo).toBe(false);
  });

  it("says when the file was removed, and nothing when there is no save", () => {
    expect(savedNotice({ ...saved, status: "undone" })?.key).toBe("chat.saveUndone");
    expect(savedNotice(undefined)).toBeNull();
    expect(savedNotice(null)).toBeNull();
  });
});

describe("withSaved", () => {
  it("replaces only that answer", () => {
    const list = [answer({ id: "m_1" }), answer({ id: "m_2" })];
    const next = withSaved(list, "m_2", saved);
    expect(next[0]).toBe(list[0]);
    expect(next[1]?.saved).toEqual(saved);
    expect(list[1]?.saved).toBeUndefined();
  });
});

it("offers .docx first", () => {
  expect(SAVE_FORMATS[0]).toBe("docx");
});
