import type { InstalledModel, SettingsView } from "./api";

export interface InstalledMatch {
  model: InstalledModel;
  inUse: boolean;
}

/** Catalog hits name their source loosely ("huggingface", "Hugging Face", "ollama"). */
function sourceKey(source: string): string {
  return source.toLowerCase().replace(/\s+/g, "").includes("huggingface")
    ? "huggingface"
    : "ollama";
}

/** The installed AI for a catalog entry, if this computer already has it. */
export function findInstalled(
  settings: Pick<SettingsView, "activeModel" | "otherModels"> | null | undefined,
  source: string,
  reference: string,
): InstalledMatch | null {
  const key = sourceKey(source);
  const same = (model: InstalledModel) =>
    sourceKey(model.source) === key && model.reference.toLowerCase() === reference.toLowerCase();
  const active = settings?.activeModel;
  if (active && same(active)) return { model: active, inUse: true };
  const other = settings?.otherModels?.find(same);
  return other ? { model: other, inUse: false } : null;
}
