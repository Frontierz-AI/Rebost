import type { ChatEvent, StoredMessage } from "./api";

export type ChatPending = {
  messageId: string;
  threadId: string;
  text: string;
  thinking: string;
  phase: "queued" | "streaming";
};

export type ChatReduceResult = {
  pending: ChatPending | null;
  append?: StoredMessage;
  error?: string;
  refreshThreads?: boolean;
};

export function reduceChatEvent(pending: ChatPending | null, event: ChatEvent): ChatReduceResult {
  switch (event.kind) {
    case "queued":
      return {
        pending: {
          messageId: event.messageId,
          threadId: event.threadId,
          text: "",
          thinking: "",
          phase: "queued",
        },
      };
    case "started": {
      const next = adopt(pending, event);
      return { pending: next ? { ...next, phase: "streaming" } : next };
    }
    case "delta": {
      const next = adopt(pending, event);
      if (next && event.text) {
        return { pending: { ...next, text: next.text + event.text, phase: "streaming" } };
      }
      return { pending: next };
    }
    case "thinking": {
      const next = adopt(pending, event);
      if (next && event.text) {
        return {
          pending: { ...next, thinking: next.thinking + event.text, phase: "streaming" },
        };
      }
      return { pending: next };
    }
    case "promote": {
      const next = pending;
      if (next) {
        return { pending: { ...next, thinking: next.thinking + next.text, text: "" } };
      }
      return { pending: next };
    }
    case "done": {
      const message = event.message;
      const append = message && message.status !== "error" ? message : undefined;
      return { pending: null, append, refreshThreads: true };
    }
    case "error":
      return { pending: null, error: event.error };
    default: {
      const _exhaustive: never = event.kind;
      return _exhaustive;
    }
  }
}

function adopt(pending: ChatPending | null, event: ChatEvent): ChatPending | null {
  if (pending && pending.messageId === event.messageId) {
    return pending;
  }
  if (event.kind === "delta" || event.kind === "thinking" || event.kind === "started") {
    return {
      messageId: event.messageId,
      threadId: event.threadId,
      text: "",
      thinking: "",
      phase: "streaming",
    };
  }
  return pending;
}
