import { api, type ImportResult } from "./api";
import { pinFileNames } from "./placeholders";
import { shelfCapMessage, skippedLongMessage } from "./shelf-cap";
import { chatState, fillDraft, notify, notifyInvokeError, refreshThreads } from "./stores.svelte";

export type ChatImportResult = "done" | "cancelled";

function addedMessage(queued: number): string {
  return queued === 1
    ? "Added to this conversation."
    : `Added ${queued} files to this conversation.`;
}

function chatImportNotice(result: ImportResult): string {
  const longMsg = skippedLongMessage(result.skippedLong ?? 0);
  if (result.atLimit) {
    const cap = shelfCapMessage(result.queued);
    return longMsg ? `${cap} ${longMsg}` : cap;
  }
  if (result.queued === 0) {
    return longMsg ?? "Those files aren't a supported type.";
  }
  return longMsg ? `${addedMessage(result.queued)} ${longMsg}` : addedMessage(result.queued);
}

async function ensureActiveThread(): Promise<string> {
  if (chatState.activeThreadId) return chatState.activeThreadId;
  const thread = await api.threadCreate(chatState.selectedShelfId);
  await refreshThreads();
  chatState.activeThreadId = thread.id;
  chatState.messages = [];
  chatState.uploadShelf = null;
  return thread.id;
}

/** Import dropped paths, or open the file picker when `paths` is omitted. */
export async function importIntoChat(paths?: string[]): Promise<ChatImportResult> {
  try {
    const fromDrop = paths !== undefined;
    if (fromDrop && paths.length === 0) return "cancelled";

    const picked = fromDrop ? paths : await api.pickFiles();
    if (!picked || picked.length === 0) return "cancelled";

    const threadId = await ensureActiveThread();
    const shelf = await api.threadEnsureUploadShelf(threadId);
    chatState.uploadShelf = shelf;
    await refreshThreads();

    const result = await api.shelfImportPaths(shelf.id, picked);
    if (result.queued > 0) {
      fillDraft(pinFileNames(chatState.draft, result.names));
    }
    notify(chatImportNotice(result));
    return "done";
  } catch (error) {
    notifyInvokeError(error);
    return "done";
  }
}
