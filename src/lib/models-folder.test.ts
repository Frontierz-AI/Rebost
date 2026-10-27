import { describe, expect, it } from "vitest";
import {
  SPARE_BYTES,
  folderLabel,
  folderName,
  folderProblemMessage,
  hasRoom,
} from "./models-folder";

describe("models folder", () => {
  it("names the folder by its last part on either platform", () => {
    expect(folderName("D:\\Rebost\\Modelos")).toBe("Modelos");
    expect(folderName("D:\\Rebost\\Modelos\\")).toBe("Modelos");
    expect(folderName("/Volumes/Data/AI")).toBe("AI");
    expect(folderName("E:\\")).toBe("E:");
  });

  it("calls the default folder Rebost's own", () => {
    expect(folderLabel({ path: "/Users/me/Library/x/models", custom: false })).toBe(
      "Rebost's own folder",
    );
    expect(folderLabel({ path: "/Volumes/Data/AI", custom: true })).toBe("AI");
  });

  it("explains a full drive with the room it needs", () => {
    const gib = 1024 ** 3;
    expect(folderProblemMessage({ problem: null, moveBytes: 0 })).toBeNull();
    expect(folderProblemMessage({ problem: "models-folder-full", moveBytes: 4 * gib })).toBe(
      "That drive doesn't have enough room. It needs about 5.0 GB free.",
    );
    expect(folderProblemMessage({ problem: "models-folder-not-writable", moveBytes: 0 })).toBe(
      "Rebost can't save files in that folder. Choose another one.",
    );
    expect(folderProblemMessage({ problem: "something-new", moveBytes: 0 })).toBe(
      "Something went wrong. Try again.",
    );
    expect(SPARE_BYTES).toBe(gib);
  });

  it("only warns about room when the free space is known", () => {
    expect(hasRoom(10, 10)).toBe(true);
    expect(hasRoom(9, 10)).toBe(false);
    expect(hasRoom(null, 10)).toBe(true);
    expect(hasRoom(0, 10)).toBe(true);
  });
});
