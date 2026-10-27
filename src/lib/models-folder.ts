/** Where AIs are kept: labels and messages shared by Settings and onboarding. */
import { formatBytes, type ModelsFolderChoice, type ModelsFolderView } from "./api";
import { t } from "./i18n.svelte";

/** Room Rebost leaves free after a move (`models_folder::SPARE_BYTES`). */
export const SPARE_BYTES = 1024 ** 3;

/** Id of a running move for `api.downloadCancel` (`engine::MOVE_ID`). */
export const MOVE_ID = "move:models";

const PROBLEM_KEYS: Record<string, string> = {
  "models-folder-not-writable": "errors.modelsFolderNotWritable",
  "models-folder-app-data": "errors.modelsFolderAppData",
  "models-folder-name-taken": "errors.modelsFolderNameTaken",
  "models-folder-no-ais": "errors.modelsFolderNoAis",
  "models-folder-missing": "errors.modelsFolderMissing",
};

/** Last part of a macOS or Windows path: `D:\Rebost\Modelos` → `Modelos`. */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.at(-1) ?? path;
}

/** Short name for the folder in use. */
export function folderLabel(folder: Pick<ModelsFolderView, "path" | "custom">): string {
  return folder.custom ? folderName(folder.path) : t("modelsFolder.defaultName");
}

/** Why a picked folder can't hold the AIs, in the person's words. */
export function folderProblemMessage(
  choice: Pick<ModelsFolderChoice, "problem" | "moveBytes">,
): string | null {
  if (!choice.problem) return null;
  if (choice.problem === "models-folder-full") {
    return t("modelsFolder.full", { size: formatBytes(choice.moveBytes + SPARE_BYTES) });
  }
  return t(PROBLEM_KEYS[choice.problem] ?? "errors.fallback");
}

/** An unknown free space never warns. */
export function hasRoom(freeBytes: number | null | undefined, neededBytes: number): boolean {
  return freeBytes == null || freeBytes <= 0 || freeBytes >= neededBytes;
}
