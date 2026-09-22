import { api, type ChatImage } from "./api";
import { app, chatState, ensureActiveThread, notify, notifyInvokeError } from "./stores.svelte";
import { t } from "./i18n.svelte";

export const MAX_IMAGE_BYTES = 20 * 1024 * 1024;

export function isImagePath(path: string): boolean {
  return /\.(png|jpe?g|webp|gif|bmp|heic|tiff?)$/i.test(path);
}

export function pastedImages(data: DataTransfer | null): File[] {
  return Array.from(data?.items ?? [])
    .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
    .map((item) => item.getAsFile())
    .filter((file): file is File => file !== null);
}

function readBase64(file: File): Promise<string> {
  if (file.size > MAX_IMAGE_BYTES) return Promise.reject(new Error(t("images.tooLarge")));
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error(t("images.invalid")));
    reader.onload = () => resolve(String(reader.result).split(",")[1] ?? "");
    reader.readAsDataURL(file);
  });
}

// A single intake queue also covers the moment a new conversation receives its ID.
let queue = Promise.resolve();

/** Both native picker and paste stage images on the captured conversation. */
export async function addChatImages(
  inputs: (File | string)[],
  capturedThread?: string,
): Promise<void> {
  if (!inputs.length) return;
  if (!app.engine.vision) {
    notify(t("images.unavailable"));
    return;
  }
  const navigation = chatState.navigation;
  const key = capturedThread ?? chatState.activeThreadId ?? "new";
  chatState.imports[key] = (chatState.imports[key] ?? 0) + 1;
  let remaining = inputs.length;
  chatState.imageImports[key] = (chatState.imageImports[key] ?? 0) + remaining;
  const work = queue
    .then(async () => {
      if (key === "new" && navigation !== chatState.navigation) return;
      const threadId = key === "new" ? await ensureActiveThread() : key;
      for (const input of inputs) {
        const limits = app.engine.vision;
        if (!limits) throw new Error(t("images.unavailable"));
        if ((chatState.imageDrafts[threadId]?.length ?? 0) >= limits.maxImages) {
          notify(t("images.limit", { count: limits.maxImages }));
          break;
        }
        const name =
          typeof input === "string"
            ? (input.split(/[\\/]/).pop() ?? "image.png")
            : t("images.pasted");
        const image = await api.chatImageAdd(
          threadId,
          name,
          typeof input === "string" ? { path: input } : { data: await readBase64(input) },
        );
        chatState.imageDrafts[threadId] = [...(chatState.imageDrafts[threadId] ?? []), image];
        chatState.imageImports[key] = Math.max(0, (chatState.imageImports[key] ?? 1) - 1);
        remaining--;
      }
    })
    .catch(notifyInvokeError)
    .finally(() => {
      chatState.imports[key] = Math.max(0, (chatState.imports[key] ?? 1) - 1);
      chatState.imageImports[key] = Math.max(0, (chatState.imageImports[key] ?? 0) - remaining);
    });
  queue = work;
  return work;
}

export function removeChatImage(threadId: string, image: ChatImage): void {
  chatState.imageDrafts[threadId] = (chatState.imageDrafts[threadId] ?? []).filter(
    (item) => item.id !== image.id,
  );
  void api.chatImageRemove(threadId, image.id).catch(notifyInvokeError);
}
