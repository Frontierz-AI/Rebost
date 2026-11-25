import type { SavedDocument, SaveFormat, StoredMessage } from "./api";

/** Formats the Save to Shelf menu offers, default first. */
export const SAVE_FORMATS: readonly SaveFormat[] = ["docx", "md", "txt"];

/** Error code the backend uses when the Shelf has reached its file limit. */
export const SHELF_FULL = "shelf-full";

export const SAVE_FORMAT_LABEL_KEYS: Record<SaveFormat, string> = {
  docx: "chat.saveFormatDocx",
  md: "chat.saveFormatMd",
  txt: "chat.saveFormatTxt",
};

/** The button shows on a finished answer when the conversation has a library Shelf. */
export function canSaveToShelf(
  message: StoredMessage,
  shelfId: string | null | undefined,
  busy = false,
): boolean {
  return (
    !busy &&
    !!shelfId &&
    message.role === "assistant" &&
    message.status === "done" &&
    message.text.trim().length > 0 &&
    message.saved?.status !== "saved"
  );
}

export interface SavedNotice {
  key: string;
  params: Record<string, string>;
  tone: "ok" | "muted" | "error";
  canOpen: boolean;
  canUndo: boolean;
}

/** What to say under an answer about its saved file. */
export function savedNotice(saved: SavedDocument | null | undefined): SavedNotice | null {
  if (!saved) return null;
  const file = saved.fileName;
  switch (saved.status) {
    case "saved":
      return {
        key: "chat.savedTo",
        params: { file },
        tone: "ok",
        canOpen: !!saved.path,
        canUndo: !!saved.path,
      };
    case "undone":
      return {
        key: "chat.saveUndone",
        params: { file },
        tone: "muted",
        canOpen: false,
        canUndo: false,
      };
    case "failed":
      return saved.error === SHELF_FULL
        ? {
            key: "chat.saveShelfFull",
            params: { file },
            tone: "error",
            canOpen: false,
            canUndo: false,
          }
        : {
            key: "chat.saveFailedNamed",
            params: { file, reason: saved.error ?? "" },
            tone: "error",
            canOpen: false,
            canUndo: false,
          };
    default: {
      const _exhaustive: never = saved.status;
      return _exhaustive;
    }
  }
}

/** Messages with one answer's saved file replaced. */
export function withSaved(
  messages: StoredMessage[],
  messageId: string,
  saved: SavedDocument,
): StoredMessage[] {
  return messages.map((message) => (message.id === messageId ? { ...message, saved } : message));
}
