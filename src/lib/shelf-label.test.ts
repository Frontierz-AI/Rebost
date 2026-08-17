import { describe, expect, it } from "vitest";
import {
  isUploadShelf,
  shelfDisplayName,
  threadShelfSubtitle,
  UPLOADED_FILES_LABEL,
} from "./shelf-label";

describe("shelfDisplayName", () => {
  it("uses Uploaded files for a conversation shelf", () => {
    expect(isUploadShelf({ threadId: "t_1" })).toBe(true);
    expect(shelfDisplayName({ name: "Files", threadId: "t_1" })).toBe(UPLOADED_FILES_LABEL);
    expect(shelfDisplayName({ name: "Legal" })).toBe("Legal");
  });
});

describe("threadShelfSubtitle", () => {
  it("labels the upload shelf without looking it up in Shelves", () => {
    expect(
      threadShelfSubtitle({ shelfId: "s_up", uploadShelfId: "s_up" }, [
        { id: "s_lib", name: "Legal" },
      ]),
    ).toBe(UPLOADED_FILES_LABEL);
    expect(
      threadShelfSubtitle({ shelfId: "s_lib", uploadShelfId: "s_up" }, [
        { id: "s_lib", name: "Legal" },
      ]),
    ).toBe("Legal");
    expect(threadShelfSubtitle({ shelfId: null }, [])).toBeNull();
    expect(threadShelfSubtitle({ shelfId: null, uploadShelfId: "s_up" }, [])).toBe(
      UPLOADED_FILES_LABEL,
    );
  });
});
