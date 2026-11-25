import { describe, expect, it } from "vitest";
import type { ThreadMeta, ThreadSearchHit } from "./api";
import { isMac } from "./platform";
import {
  LatestRequest,
  foldForSearch,
  isListFindShortcut,
  moreMatches,
  queryWords,
  searchGroups,
  searchKeyAction,
  titleMatches,
} from "./thread-search";

function thread(id: string, title: string): ThreadMeta {
  return {
    id,
    title,
    createdAt: "2026-11-01T10:00:00+00:00",
    updatedAt: "2026-11-01T10:00:00+00:00",
    messageCount: 2,
    avatarId: "apple",
  };
}

function hit(threadId: string, matches = 1, matchesCapped = false): ThreadSearchHit {
  return {
    threadId,
    messageId: `m_${threadId}`,
    role: "assistant",
    snippet: [{ text: "quote", mark: true }],
    createdAt: "2026-11-01T10:00:00+00:00",
    matches,
    matchesCapped,
  };
}

function key(
  key: string,
  mods: { meta?: boolean; ctrl?: boolean; alt?: boolean; shift?: boolean } = {},
): KeyboardEvent {
  return {
    key,
    metaKey: !!mods.meta,
    ctrlKey: !!mods.ctrl,
    altKey: !!mods.alt,
    shiftKey: !!mods.shift,
    repeat: false,
  } as KeyboardEvent;
}

describe("title filter", () => {
  it("ignores case and accents", () => {
    expect(foldForSearch("Pressupost d'ÀLEX")).toBe("pressupost d'alex");
    expect(titleMatches("Vacances d'estiu", queryWords("VACANCES"))).toBe(true);
    expect(titleMatches("Reunió amb l'Àlex", queryWords("reunio alex"))).toBe(true);
    expect(titleMatches("Canción de cuna", queryWords("cancion"))).toBe(true);
    expect(titleMatches("Supplier quotes", queryWords("quotes plumber"))).toBe(false);
  });

  it("matches nothing for a blank query", () => {
    expect(queryWords("   ")).toEqual([]);
    expect(titleMatches("Anything", [])).toBe(false);
  });
});

describe("searchGroups", () => {
  const threads = [
    thread("t_1", "Supplier quote"),
    thread("t_2", "Kitchen"),
    thread("t_3", "Quote comparison"),
  ];

  it("lists titles first and message-only matches after, each conversation once", () => {
    const groups = searchGroups(threads, "quote", [hit("t_2"), hit("t_3", 3)]);
    expect(groups.titles.map((row) => row.thread.id)).toEqual(["t_1", "t_3"]);
    expect(groups.titles[0]?.hit).toBeNull();
    expect(groups.titles[1]?.hit?.matches).toBe(3);
    expect(groups.messages.map((row) => row.thread.id)).toEqual(["t_2"]);
  });

  it("keeps title order when snippets arrive", () => {
    const before = searchGroups(threads, "quote", []);
    const after = searchGroups(threads, "quote", [hit("t_3"), hit("t_1")]);
    expect(after.titles.map((row) => row.thread.id)).toEqual(
      before.titles.map((row) => row.thread.id),
    );
  });

  it("drops hits for conversations that are gone", () => {
    const groups = searchGroups(threads, "kitchen", [hit("t_deleted"), hit("t_2")]);
    expect(groups.messages.map((row) => row.thread.id)).toEqual([]);
    expect(groups.titles.map((row) => row.thread.id)).toEqual(["t_2"]);
  });

  it("is empty for a blank query", () => {
    expect(searchGroups(threads, " ", [hit("t_1")])).toEqual({ titles: [], messages: [] });
  });
});

describe("moreMatches", () => {
  it("counts the other matches and flags a capped count", () => {
    expect(moreMatches(null)).toBeNull();
    expect(moreMatches(hit("t", 1))).toBeNull();
    expect(moreMatches(hit("t", 4))).toEqual({ count: 3, atLeast: false });
    expect(moreMatches(hit("t", 4, true))).toEqual({ count: 3, atLeast: true });
  });
});

describe("LatestRequest", () => {
  it("ignores responses that arrive out of order", () => {
    const requests = new LatestRequest();
    const shown: string[] = [];
    const cont = requests.begin();
    const contract = requests.begin();
    const answer = (request: number, value: string) => {
      if (requests.isCurrent(request)) shown.push(value);
    };
    answer(contract, "contract");
    answer(cont, "cont");
    expect(shown).toEqual(["contract"]);
  });

  it("drops everything in flight after cancel", () => {
    const requests = new LatestRequest();
    const pending = requests.begin();
    requests.cancel();
    expect(requests.isCurrent(pending)).toBe(false);
  });
});

describe("searchKeyAction", () => {
  it("clears on the first Escape and leaves on the second", () => {
    expect(searchKeyAction("Escape", "quote", 0, 3)).toEqual({ kind: "clear" });
    expect(searchKeyAction("Escape", "", -1, 0)).toEqual({ kind: "leave" });
  });

  it("moves through results with the arrow keys and stays in range", () => {
    expect(searchKeyAction("ArrowDown", "q", -1, 3)).toEqual({ kind: "move", index: 0 });
    expect(searchKeyAction("ArrowDown", "q", 2, 3)).toEqual({ kind: "move", index: 2 });
    expect(searchKeyAction("ArrowUp", "q", 2, 3)).toEqual({ kind: "move", index: 1 });
    expect(searchKeyAction("ArrowUp", "q", 0, 3)).toEqual({ kind: "move", index: 0 });
    expect(searchKeyAction("ArrowDown", "q", -1, 0)).toBeNull();
  });

  it("opens the highlighted result on Enter, or the first one", () => {
    expect(searchKeyAction("Enter", "q", 1, 3)).toEqual({ kind: "open", index: 1 });
    expect(searchKeyAction("Enter", "q", -1, 3)).toEqual({ kind: "open", index: 0 });
    expect(searchKeyAction("Enter", "q", 5, 3)).toEqual({ kind: "open", index: 0 });
    expect(searchKeyAction("Enter", "q", -1, 0)).toBeNull();
    expect(searchKeyAction("a", "q", 0, 3)).toBeNull();
  });

  it("opens the right conversation across both groups", () => {
    const threads = [thread("t_1", "Quote"), thread("t_2", "Kitchen")];
    const groups = searchGroups(threads, "quote", [hit("t_2")]);
    const rows = [...groups.titles, ...groups.messages];
    let active = -1;
    for (const pressed of ["ArrowDown", "ArrowDown"]) {
      const action = searchKeyAction(pressed, "quote", active, rows.length);
      if (action?.kind === "move") active = action.index;
    }
    const action = searchKeyAction("Enter", "quote", active, rows.length);
    expect(action?.kind === "open" ? rows[action.index]?.thread.id : null).toBe("t_2");
  });
});

describe("isListFindShortcut", () => {
  it("accepts modifier+F only", () => {
    const mod = isMac() ? { meta: true } : { ctrl: true };
    expect(isListFindShortcut(key("f", mod))).toBe(true);
    expect(isListFindShortcut(key("F", { ...mod, shift: true }))).toBe(false);
    expect(isListFindShortcut(key("f", { ...mod, alt: true }))).toBe(false);
    expect(isListFindShortcut(key("f"))).toBe(false);
  });
});
