import type { ThreadMeta, ThreadSearchHit } from "./api";
import { isModKey } from "./shortcuts";

/** Lowercase without accents: "Àlex" and "alex" compare equal. */
export function foldForSearch(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{M}+/gu, "")
    .toLowerCase();
}

/** Folded query words; empty when the query is blank. */
export function queryWords(query: string): string[] {
  return foldForSearch(query).split(/\s+/u).filter(Boolean);
}

/** Every query word appears somewhere in the title. */
export function titleMatches(title: string, words: string[]): boolean {
  if (words.length === 0) return false;
  const folded = foldForSearch(title);
  return words.every((word) => folded.includes(word));
}

export interface SearchRow {
  thread: ThreadMeta;
  /** Best message match, once the backend answers. */
  hit: ThreadSearchHit | null;
}

export interface SearchGroups {
  /** Title matches, in conversation-list order; a message hit adds its snippet in place. */
  titles: SearchRow[];
  /** Conversations that match only by message text, best first. */
  messages: SearchRow[];
}

/**
 * Titles first (instant, client-side), then message-only matches. A
 * conversation appears once. Hits for conversations no longer in the list
 * are dropped.
 */
export function searchGroups(
  threads: ThreadMeta[],
  query: string,
  hits: ThreadSearchHit[],
): SearchGroups {
  const words = queryWords(query);
  if (words.length === 0) return { titles: [], messages: [] };
  const hitByThread = new Map(hits.map((hit) => [hit.threadId, hit]));
  const titles: SearchRow[] = [];
  const titled = new Set<string>();
  for (const thread of threads) {
    if (!titleMatches(thread.title, words)) continue;
    titled.add(thread.id);
    titles.push({ thread, hit: hitByThread.get(thread.id) ?? null });
  }
  const byId = new Map(threads.map((thread) => [thread.id, thread]));
  const messages: SearchRow[] = [];
  for (const hit of hits) {
    const thread = byId.get(hit.threadId);
    if (!thread || titled.has(thread.id)) continue;
    messages.push({ thread, hit });
  }
  return { titles, messages };
}

/** Other matching messages beyond the one shown; `atLeast` when the count may be low. */
export function moreMatches(
  hit: ThreadSearchHit | null,
): { count: number; atLeast: boolean } | null {
  if (!hit) return null;
  const count = hit.matches - 1;
  if (count <= 0) return null;
  return { count, atLeast: hit.matchesCapped };
}

/**
 * Hands out request numbers so a slow answer for "cont" cannot replace the
 * results for "contract".
 */
export class LatestRequest {
  private last = 0;

  begin(): number {
    this.last += 1;
    return this.last;
  }

  isCurrent(request: number): boolean {
    return request === this.last;
  }

  /** Invalidate whatever is in flight (the query was cleared). */
  cancel(): void {
    this.last += 1;
  }
}

export type SearchKeyAction =
  | { kind: "move"; index: number }
  | { kind: "open"; index: number }
  | { kind: "clear" }
  | { kind: "leave" }
  | null;

/**
 * Keys in the search field. `active` is the highlighted result (-1 = none),
 * `count` how many results there are.
 */
export function searchKeyAction(
  key: string,
  query: string,
  active: number,
  count: number,
): SearchKeyAction {
  switch (key) {
    case "ArrowDown":
      return count > 0 ? { kind: "move", index: Math.min(active + 1, count - 1) } : null;
    case "ArrowUp":
      return count > 0 ? { kind: "move", index: Math.max(active - 1, 0) } : null;
    case "Enter":
      if (count === 0) return null;
      return { kind: "open", index: active >= 0 && active < count ? active : 0 };
    case "Escape":
      return query.length > 0 ? { kind: "clear" } : { kind: "leave" };
    default:
      return null;
  }
}

/** ⌘F / Ctrl+F (no Shift or Option) while the conversation list has focus. */
export function isListFindShortcut(event: KeyboardEvent): boolean {
  return (
    isModKey(event) &&
    !event.shiftKey &&
    !event.altKey &&
    !event.repeat &&
    (event.key === "f" || event.key === "F")
  );
}

/** Wait this long after typing stops before asking the message index. */
export const SEARCH_DEBOUNCE_MS = 150;
