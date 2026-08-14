import { describe, expect, it } from "vitest";
import { reduceChatEvent, type ChatPending } from "./chat-reducer";
import type { ChatEvent, StoredMessage } from "./api";

function base(kind: ChatEvent["kind"], extra: Partial<ChatEvent> = {}): ChatEvent {
  return {
    threadId: "t1",
    messageId: "m1",
    kind,
    ...extra,
  };
}

function pending(extra: Partial<ChatPending> = {}): ChatPending {
  return {
    messageId: "m1",
    threadId: "t1",
    text: "",
    thinking: "",
    phase: "queued",
    ...extra,
  };
}

const doneMessage: StoredMessage = {
  id: "m1",
  role: "assistant",
  text: "Hello world",
  ts: "now",
  sources: [],
  status: "done",
};

describe("reduceChatEvent", () => {
  it("opens a queued pending message", () => {
    const out = reduceChatEvent(null, base("queued"));
    expect(out.pending?.phase).toBe("queued");
    expect(out.pending?.messageId).toBe("m1");
    expect(out.pending?.text).toBe("");
  });

  it("started ends the warming-up queued phase", () => {
    const out = reduceChatEvent(pending(), base("started"));
    expect(out.pending?.phase).toBe("streaming");
    expect(out.pending?.messageId).toBe("m1");
    expect(out.error).toBeUndefined();
  });

  it("appends deltas then clears on done", () => {
    let next: ChatPending | null = reduceChatEvent(null, base("queued")).pending;
    next = reduceChatEvent(next, base("started")).pending;
    next = reduceChatEvent(next, base("delta", { text: "Hello" })).pending;
    next = reduceChatEvent(next, base("delta", { text: " world" })).pending;
    expect(next?.text).toBe("Hello world");
    expect(next?.phase).toBe("streaming");
    const done = reduceChatEvent(next, base("done", { message: doneMessage }));
    expect(done.pending).toBeNull();
    expect(done.append?.text).toBe("Hello world");
    expect(done.refreshThreads).toBe(true);
  });

  it("appends thinking deltas separately from the answer", () => {
    let next: ChatPending | null = pending({ phase: "streaming" });
    next = reduceChatEvent(next, base("thinking", { text: "hmm " })).pending;
    next = reduceChatEvent(next, base("thinking", { text: "ok" })).pending;
    expect(next?.thinking).toBe("hmm ok");
    expect(next?.text).toBe("");
  });

  it("promotes answer text into thinking", () => {
    const out = reduceChatEvent(
      pending({ text: "scratch", thinking: "pre ", phase: "streaming" }),
      base("promote"),
    );
    expect(out.pending?.thinking).toBe("pre scratch");
    expect(out.pending?.text).toBe("");
  });

  it("clears pending and surfaces an engine error", () => {
    const out = reduceChatEvent(
      pending({ phase: "streaming", text: "partial" }),
      base("error", { error: "The answer couldn't be generated. Try again." }),
    );
    expect(out.pending).toBeNull();
    expect(out.append).toBeUndefined();
    expect(out.error).toBe("The answer couldn't be generated. Try again.");
  });

  it("treats a cancelled or failed done as no append", () => {
    const stopped = reduceChatEvent(
      pending({ phase: "streaming" }),
      base("done", { message: { ...doneMessage, status: "error" } }),
    );
    expect(stopped.pending).toBeNull();
    expect(stopped.append).toBeUndefined();
    expect(stopped.refreshThreads).toBe(true);

    const cancelled = reduceChatEvent(
      pending({ phase: "streaming" }),
      base("done", { message: { ...doneMessage, status: "stopped", text: "half" } }),
    );
    expect(cancelled.append?.status).toBe("stopped");
    expect(cancelled.append?.text).toBe("half");
  });

  it("adopts a late started or delta when pending is missing", () => {
    const started = reduceChatEvent(null, base("started"));
    expect(started.pending).toEqual({
      messageId: "m1",
      threadId: "t1",
      text: "",
      thinking: "",
      phase: "streaming",
    });

    const delta = reduceChatEvent(null, base("delta", { text: "Hi" }));
    expect(delta.pending?.text).toBe("Hi");
    expect(delta.pending?.phase).toBe("streaming");
  });

  it("ignores deltas for a different in-flight message", () => {
    const current = pending({ phase: "streaming", text: "keep" });
    const out = reduceChatEvent(current, base("delta", { messageId: "m2", text: "other" }));
    expect(out.pending?.messageId).toBe("m2");
    expect(out.pending?.text).toBe("other");
  });

  it("promote and empty delta do nothing without pending text", () => {
    expect(reduceChatEvent(null, base("promote")).pending).toBeNull();
    const same = pending({ phase: "streaming", text: "x" });
    expect(reduceChatEvent(same, base("delta")).pending?.text).toBe("x");
  });
});
