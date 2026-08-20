import type { ModelSearchResult } from "$lib/api";

export const EXPLORE_PAGE_SIZE = 50;
export const MODEL_BUDGET_FRACTION = 0.55;
const GIB = 1024 * 1024 * 1024;

export type ExploreSort = "best" | "released" | "size" | "downloads";
export type ExploreSortDir = "asc" | "desc";
export type ExploreColumn = "released" | "size" | "downloads";

export const EXPLORE_SORTS: { id: ExploreSort; label: string }[] = [
  { id: "best", label: "Best for this computer" },
  { id: "released", label: "Newest" },
  { id: "size", label: "Smallest download" },
  { id: "downloads", label: "Most downloaded" },
];

export function defaultExploreSortDir(sort: ExploreSort): ExploreSortDir {
  switch (sort) {
    case "best":
    case "released":
    case "downloads":
      return "desc";
    case "size":
      return "asc";
    default: {
      const _never: never = sort;
      return _never;
    }
  }
}

export function nextExploreSort(
  current: ExploreSort,
  currentDir: ExploreSortDir,
  next: ExploreSort,
): { sort: ExploreSort; dir: ExploreSortDir } {
  if (next === "best") return { sort: "best", dir: "desc" };
  if (current === next) {
    return { sort: next, dir: currentDir === "asc" ? "desc" : "asc" };
  }
  return { sort: next, dir: defaultExploreSortDir(next) };
}

export function chipSortActive(chip: ExploreSort, sort: ExploreSort, dir: ExploreSortDir): boolean {
  if (chip === "best") return sort === "best";
  return sort === chip && dir === defaultExploreSortDir(chip);
}

export function modelBudgetBytes(totalRamBytes?: number): number | undefined {
  if (totalRamBytes == null || totalRamBytes <= 0) return undefined;
  return Math.floor(totalRamBytes * MODEL_BUDGET_FRACTION);
}

export function runtimeNeedBytes(fileBytes: number): number {
  return fileBytes * 1.15 + 2 * GIB;
}

function releasedTime(released?: string): number {
  if (!released) return Number.NaN;
  const raw = released.length === 7 ? `${released}-01` : released;
  return Date.parse(raw);
}

function utcDay(ms: number): number {
  const date = new Date(ms);
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate());
}

export function daysSinceRelease(released: string | undefined, now: number): number | null {
  const time = releasedTime(released);
  if (Number.isNaN(time)) return null;
  return Math.round((utcDay(now) - utcDay(time)) / 86_400_000);
}

export function isRecentRelease(released: string | undefined, now: number): boolean {
  const days = daysSinceRelease(released, now);
  return days != null && days >= 0 && days <= 90;
}

function recencyScore(released: string | undefined, now: number): number {
  const days = daysSinceRelease(released, now);
  if (days == null || days < 0 || days > 365) return 0;
  if (days <= 90) return 100 - Math.floor(days / 2);
  if (days <= 180) return 42;
  return 16;
}

function usageScore(sizeBytes: number | undefined, budgetBytes: number | undefined): number {
  if (sizeBytes == null || budgetBytes == null || budgetBytes <= 0) return 32;
  const need = runtimeNeedBytes(sizeBytes);
  if (need > budgetBytes) return 0;
  const ratio = need / budgetBytes;
  if (ratio >= 0.45 && ratio <= 0.85) return 100;
  if (ratio > 0.85) return 78;
  if (ratio >= 0.3) return 68;
  if (ratio >= 0.15) return 44;
  return 22;
}

function downloadScore(downloads?: number): number {
  return Math.min(48, Math.round(Math.log10((downloads ?? 0) + 1) * 8));
}

function fitScore(fits?: boolean): number {
  if (fits === true) return 120;
  if (fits === false) return 0;
  return 28;
}

export function bestExploreScore(
  result: ModelSearchResult,
  now: number,
  budgetBytes?: number,
): number {
  if (result.fits === false) return downloadScore(result.downloads);
  return (
    fitScore(result.fits) +
    recencyScore(result.released, now) +
    usageScore(result.sizeBytes, budgetBytes) +
    (result.official ? 48 : 0) +
    downloadScore(result.downloads)
  );
}

function byName(a: ModelSearchResult, b: ModelSearchResult): number {
  return a.name.localeCompare(b.name);
}

function compareOptionalNumber(
  left: number | undefined,
  right: number | undefined,
  dir: ExploreSortDir,
): number {
  if (left == null && right == null) return 0;
  if (left == null) return 1;
  if (right == null) return -1;
  return dir === "asc" ? left - right : right - left;
}

export function sortExploreResults(
  results: ModelSearchResult[],
  sort: ExploreSort,
  now = Date.now(),
  budgetBytes?: number,
  dir: ExploreSortDir = defaultExploreSortDir(sort),
): ModelSearchResult[] {
  const copy = results.slice();
  switch (sort) {
    case "best":
      copy.sort((a, b) => {
        const score = bestExploreScore(b, now, budgetBytes) - bestExploreScore(a, now, budgetBytes);
        if (score !== 0) return score;
        const released = (b.released ?? "").localeCompare(a.released ?? "");
        if (released !== 0) return released;
        return byName(a, b);
      });
      return copy;
    case "released":
      copy.sort((a, b) => {
        const left = a.released ?? "";
        const right = b.released ?? "";
        if (!left && !right) return byName(a, b);
        if (!left) return 1;
        if (!right) return -1;
        const released = dir === "asc" ? left.localeCompare(right) : right.localeCompare(left);
        if (released !== 0) return released;
        return byName(a, b);
      });
      return copy;
    case "size":
      copy.sort((a, b) => {
        const size = compareOptionalNumber(a.sizeBytes, b.sizeBytes, dir);
        if (size !== 0) return size;
        return byName(a, b);
      });
      return copy;
    case "downloads":
      copy.sort((a, b) => {
        const downloads = compareOptionalNumber(a.downloads, b.downloads, dir);
        if (downloads !== 0) return downloads;
        return byName(a, b);
      });
      return copy;
    default: {
      const _exhaustive: never = sort;
      return _exhaustive;
    }
  }
}

export function visibleExploreCount(
  total: number,
  page: number,
  pageSize = EXPLORE_PAGE_SIZE,
): number {
  return Math.min(total, Math.max(page, 1) * pageSize);
}

export function columnAriaSort(
  column: ExploreColumn,
  sort: ExploreSort,
  dir: ExploreSortDir = defaultExploreSortDir(sort),
): "ascending" | "descending" | "none" {
  if (sort !== column) return "none";
  return dir === "asc" ? "ascending" : "descending";
}
