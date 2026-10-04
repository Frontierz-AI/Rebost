import { describe, expect, it } from "vitest";
import type { InstalledModel } from "./api";
import { findInstalled } from "./installed-models";

const model = (reference: string, source = "huggingface"): InstalledModel => ({
  file: `${reference.split("/").pop()}.gguf`,
  name: reference,
  source,
  reference,
  sizeBytes: 1,
});

describe("findInstalled", () => {
  const settings = {
    activeModel: model("google/gemma-4-12b-GGUF"),
    otherModels: [model("qwen3.8:27b", "ollama")],
  };

  it("marks the active AI as in use, ignoring case", () => {
    expect(findInstalled(settings, "huggingface", "Google/Gemma-4-12B-GGUF")?.inUse).toBe(true);
  });

  it("finds other installed AIs by source and reference", () => {
    const match = findInstalled(settings, "ollama", "qwen3.8:27b");
    expect(match?.inUse).toBe(false);
    expect(match?.model.file).toBe("qwen3.8:27b.gguf");
    expect(findInstalled(settings, "huggingface", "qwen3.8:27b")).toBeNull();
  });

  it("returns null without settings or a match", () => {
    expect(findInstalled(null, "huggingface", "google/gemma-4-12b-GGUF")).toBeNull();
    expect(findInstalled(settings, "huggingface", "org/other")).toBeNull();
  });
});
