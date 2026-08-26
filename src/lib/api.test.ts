import { describe, expect, it } from "vitest";
import {
  catalogHostLabel,
  downloadErrorMessage,
  formatCount,
  PII_EMPTY_HINT,
  piiLabel,
  userFacingError,
} from "./api";

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

describe("catalogHostLabel", () => {
  it("names Hugging Face when both catalogs list the AI", () => {
    expect(catalogHostLabel("huggingface")).toBe("Hugging Face");
    expect(catalogHostLabel("huggingface+ollama")).toBe("Hugging Face");
    expect(catalogHostLabel("ollama")).toBe("Ollama");
  });
});

describe("userFacingError", () => {
  it("maps install and start failures without naming the machinery", () => {
    expect(userFacingError("no AI model installed yet")).toBe(
      "Rebost needs an AI before it can answer.",
    );
    expect(userFacingError("model file must be .gguf")).toBe(
      "That AI isn't available. Try another.",
    );
    expect(userFacingError("SHA-256 mismatch: got abcd, expected efgh")).toBe(
      "The download couldn't be verified. Try again.",
    );
    expect(userFacingError("llama-server exited early (1)")).toBe(
      "Rebost isn't ready yet. Try again in a moment.",
    );
    expect(userFacingError("engine archive did not contain llama-server")).toBe(
      "Rebost isn't ready yet. Try again in a moment.",
    );
    expect(userFacingError("generation stalled")).toBe(
      "Rebost couldn't finish that answer. Try again.",
    );
    expect(userFacingError("empty generation")).toBe(
      "Rebost couldn't finish that answer. Try again.",
    );
    expect(userFacingError("warmup-failed")).toBe(
      "That AI didn't start. Try again, or pick a smaller one.",
    );
  });

  it("keeps quiet product copy and hides leftover pins", () => {
    expect(userFacingError("A Shelf needs a name.")).toBe("A Shelf needs a name.");
    expect(userFacingError("thread 'main' panicked at src/engine/ready.rs:120")).toBe(
      "Something went wrong. Try again.",
    );
  });
});

describe("downloadErrorMessage", () => {
  it("stays quiet when the user cancelled", () => {
    expect(downloadErrorMessage("cancelled")).toBeNull();
  });

  it("keeps the previous AI when the new one does not start", () => {
    expect(downloadErrorMessage("switch-failed")).toContain("previous");
  });

  it("asks to retry or pick a smaller AI when the first one does not start", () => {
    expect(downloadErrorMessage("warmup-failed")).toContain("smaller");
  });

  it("names an incompatible AI without the file format", () => {
    expect(downloadErrorMessage("incompatible-format")).toBe(
      "This AI uses a format Rebost can't run. Pick another.",
    );
    expect(userFacingError("incompatible-format")).toBe(
      "This AI uses a format Rebost can't run. Pick another.",
    );
  });
});

describe("Privacy Lens labels", () => {
  it("names the empty-state categories without calling the file clean", () => {
    expect(PII_EMPTY_HINT).toMatch(/Social Security/);
    expect(PII_EMPTY_HINT).toMatch(/labeled names/);
    expect(PII_EMPTY_HINT.toLowerCase()).not.toContain("no personal information detected");
  });

  it("labels Social Security numbers", () => {
    expect(piiLabel("ssn", 1)).toBe("Social Security number");
    expect(piiLabel("ssn", 2)).toBe("Social Security numbers");
  });
});

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
