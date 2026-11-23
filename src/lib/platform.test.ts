import { describe, expect, it } from "vitest";
import { osFamily, supportsMica } from "./platform";

describe("osFamily", () => {
  it("falls back to the user agent when the OS plugin is missing", () => {
    const ua = globalThis.navigator?.userAgent ?? "";
    const family = osFamily();
    if (ua.includes("Windows")) expect(family).toBe("windows");
    else if (ua.includes("Mac")) expect(family).toBe("macos");
    else expect(["macos", "windows", "other"]).toContain(family);
  });
});

describe("supportsMica", () => {
  it("needs Windows 11 (build 22000 or later)", () => {
    expect(supportsMica("10.0.22631")).toBe(true);
    expect(supportsMica("10.0.22000")).toBe(true);
    expect(supportsMica("10.0.19045")).toBe(false);
    expect(supportsMica("")).toBe(false);
  });
});
