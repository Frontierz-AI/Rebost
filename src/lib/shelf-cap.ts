import { dateLocale, t } from "./i18n.svelte";

/** Files a Shelf reads unless its owner picks another limit. */
export const DEFAULT_FILE_LIMIT = 1_000;

/** The limits a Shelf can pick. Matches `FILE_LIMIT_CHOICES` in Rust. */
export const FILE_LIMIT_CHOICES = [1_000, 2_500, 5_000, 10_000] as const;

export const MAX_FILE_LIMIT = 10_000;

/** A file count in the app's language, like 2,500 or 2.500. */
export function formatCount(n: number): string {
  return n.toLocaleString(dateLocale());
}

/**
 * Toast when a Shelf hit its file limit. `adjustable` is false for a
 * conversation's attached files, which have no setting to raise.
 */
export function shelfCapMessage(
  queued: number,
  limit = DEFAULT_FILE_LIMIT,
  adjustable = true,
): string {
  const vars = { count: formatCount(queued), limit: formatCount(limit) };
  if (!adjustable) {
    if (queued <= 0) return t("imports.capFull", vars);
    const added = t(queued === 1 ? "imports.addedOne" : "imports.addedMany", vars);
    return `${added} ${t("imports.capRest", vars)}`;
  }
  const canRaise = limit < MAX_FILE_LIMIT;
  const next = canRaise ? t("shelves.capRaise", { setting: t("shelves.fileLimit") }) : "";
  if (queued <= 0) {
    return `${t("shelves.capFull", vars)} ${next || t("shelves.capRemove")}`;
  }
  const added = t(queued === 1 ? "shelves.capAddedOne" : "shelves.capAddedMany", vars);
  return next ? `${added} ${next}` : added;
}

export function skippedLongMessage(n: number): string | null {
  if (n <= 0) return null;
  if (n === 1) {
    return "That file wasn't copied; the path is too long. Add the folder instead, or use a shorter name.";
  }
  return `${n.toLocaleString("en-US")} files weren't copied; their paths are too long. Add the folder instead, or use a shorter name.`;
}

/** Toast for an import or Add folder result. Null when nothing needs saying. */
export function importFeedback(
  queued: number,
  atLimit: boolean,
  skippedLong = 0,
  limit = DEFAULT_FILE_LIMIT,
): string | null {
  const longMsg = skippedLongMessage(skippedLong);
  if (atLimit) {
    const cap = shelfCapMessage(queued, limit);
    return longMsg ? `${cap} ${longMsg}` : cap;
  }
  if (queued === 0) {
    return longMsg ?? "Those files aren't a supported type.";
  }
  if (longMsg && queued === 1) return `Added 1 file. ${longMsg}`;
  if (longMsg) {
    return `Added ${queued.toLocaleString("en-US")} files. ${longMsg}`;
  }
  return null;
}

/** Files accepted but not yet being read. Empty when none. */
export function waitingMessage(n: number): string {
  if (n <= 0) return "";
  if (n === 1) return "1 file waiting";
  return `${n.toLocaleString("en-US")} files waiting`;
}
