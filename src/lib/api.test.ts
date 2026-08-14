import { describe, expect, it } from "vitest";
import { formatCount } from "./api";

describe("formatCount", () => {
  it("compacts Hugging Face download totals", () => {
    expect(formatCount(0)).toBe("0");
    expect(formatCount(999)).toBe("999");
    expect(formatCount(1200)).toBe("1.2k");
    expect(formatCount(12_000)).toBe("12k");
    expect(formatCount(1_200_000)).toBe("1.2M");
    expect(formatCount(12_000_000)).toBe("12M");
  });
});
