import { afterEach, describe, expect, it } from "vitest";
import { applyLocale } from "./i18n.svelte";
import {
  catalogHostLabel,
  downloadErrorMessage,
  formatCount,
  piiEmptyHint,
  piiLabel,
  runsEmulatedOnArm,
  userFacingError,
} from "./api";

describe("runsEmulatedOnArm", () => {
  it("flags only the x64 copy on an ARM PC", () => {
    expect(runsEmulatedOnArm({ processArch: "x86_64", osArch: "aarch64" })).toBe(true);
    expect(runsEmulatedOnArm({ processArch: "aarch64", osArch: "aarch64" })).toBe(false);
    expect(runsEmulatedOnArm({ processArch: "x86_64", osArch: "x86_64" })).toBe(false);
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

  it("tells a failed image start apart from an AI without images", () => {
    expect(userFacingError("image-start-failed")).toBe(
      "This AI's image reading didn't start on this computer, so Rebost removed it. Chat still works with text.",
    );
    expect(userFacingError("image-unavailable")).toBe(
      "Image input is unavailable with this AI on this computer. Choose another AI to attach images.",
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
  afterEach(() => applyLocale("en"));

  it("limits the empty-state claim to information Rebost recognizes", () => {
    expect(piiEmptyHint()).toContain("it recognizes");
  });

  it("identifies the country of Social Security numbers", () => {
    expect(piiLabel("ssn", 1)).toBe("US Social Security number");
    expect(piiLabel("ssn", 2)).toBe("US Social Security numbers");
    applyLocale("nl");
    expect(piiLabel("ssn", 1)).toBe("Amerikaans socialezekerheidsnummer");
  });

  it("uses Czech count forms for one, two to four, and other totals", () => {
    applyLocale("cs");
    expect(piiLabel("email", 1)).toBe("e-mailová adresa");
    for (const count of [2, 3, 4]) {
      expect(piiLabel("email", count)).toBe("e-mailové adresy");
    }
    for (const count of [0, 5, 11, 21, 22, 104]) {
      expect(piiLabel("email", count)).toBe("e-mailových adres");
    }
    expect(piiLabel("phone", 2)).toBe("telefonní čísla");
    expect(piiLabel("phone", 5)).toBe("telefonních čísel");
  });

  it("keeps Finnish count forms and Japanese invariant labels", () => {
    applyLocale("fi");
    expect(piiLabel("name", 1)).toBe("nimi");
    expect(piiLabel("name", 2)).toBe("nimeä");
    applyLocale("ja");
    expect(piiLabel("name", 1)).toBe("氏名");
    expect(piiLabel("name", 5)).toBe("氏名");
    expect(piiLabel("unknown_category", 5)).toBe("unknown_category");
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
