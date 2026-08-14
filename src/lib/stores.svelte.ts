// Global app state (Svelte 5 runes) fed by backend events.
import {
  api,
  events,
  type AppUpdate,
  type ChatEvent,
  type DownloadEvent,
  type EngineStatus,
  type SettingsView,
  type ShelfView,
  type StoredMessage,
  type ThreadMeta,
} from "./api";
import { reduceChatEvent } from "./chat-reducer";

export type View = "chat" | "shelves" | "recipes" | "settings";

export const app = $state({
  ready: false,
  view: "chat" as View,
  onboarding: false,
  shelves: [] as ShelfView[],
  threads: [] as ThreadMeta[],
  engine: { state: "no-model" } as EngineStatus,
  settings: null as SettingsView | null,
  downloads: {} as Record<string, DownloadEvent>,
  /// Shelf the user is currently looking at (Shelves view).
  openShelfId: null as string | null,
  /// Open the new-shelf form when the Shelves view next mounts.
  createShelf: false,
  /// Bump to make shelf views refetch documents.
  ingestTick: 0,
  /// Set only when a newer GitHub release is confirmed. Never set on a failed check.
  update: null as AppUpdate | null,
});

export const chatState = $state({
  activeThreadId: null as string | null,
  messages: [] as StoredMessage[],
  /// Composer draft — lives here so Recipes can pre-fill it.
  draft: "",
  /// Bumped when a Recipe lands a prompt, so the composer grabs focus.
  draftFocus: 0,
  // In-flight assistant message (streaming).
  pending: null as null | {
    messageId: string;
    threadId: string;
    text: string;
    thinking: string;
    phase: "queued" | "streaming";
  },
});

/** A Recipe was clicked: start a fresh conversation with the prompt
 *  pre-filled, ready to complete and send. */
export function startRecipe(prompt: string) {
  chatState.activeThreadId = null;
  chatState.messages = [];
  chatState.pending = null;
  chatState.draft = prompt;
  chatState.draftFocus += 1;
  app.view = "chat";
}

export async function refreshShelves() {
  app.shelves = await api.shelvesList();
}

export async function refreshThreads() {
  app.threads = await api.threadsList();
}

export async function refreshSettings() {
  app.settings = await api.settingsGet();
}

/** Show the progress UI on the same click as Install, before the backend resolves the file. */
export function beginModelInstall(
  source: string,
  reference: string,
  name: string,
  license?: string,
  total?: number | null,
) {
  const id = `model:${reference}`;
  app.downloads = {
    ...app.downloads,
    [id]: {
      kind: "model",
      id,
      name,
      received: 0,
      total: total ?? null,
      done: false,
      phase: "downloading",
    },
  };
  return api.modelInstall(source, reference, name, license);
}

export async function openThread(threadId: string) {
  chatState.activeThreadId = threadId;
  chatState.messages = await api.threadMessages(threadId);
}

export function openCreateShelf() {
  app.openShelfId = null;
  app.createShelf = true;
  app.view = "shelves";
}

function handleChatEvent(event: ChatEvent) {
  const result = reduceChatEvent(chatState.pending, event);
  chatState.pending = result.pending;
  if (result.append && chatState.activeThreadId === event.threadId) {
    chatState.messages = [...chatState.messages, result.append];
  }
  if (result.refreshThreads) {
    refreshThreads();
  }
  if (result.error) {
    notify(result.error);
  }
}

let notifier: (message: string) => void = () => {};
export function setNotifier(fn: (message: string) => void) {
  notifier = fn;
}
export function notify(message: string) {
  notifier(message);
}

/** Wire events + initial data. Called once from App.svelte. */
export async function bootstrap() {
  await Promise.all([refreshShelves(), refreshThreads(), refreshSettings()]);
  const engineStatus = await api.engineStatus();
  app.engine = engineStatus;
  app.onboarding = !(app.settings?.onboardingDone || app.settings?.activeModel);

  // Chat is home: reopen the most recent conversation.
  if (app.threads.length > 0) {
    openThread(app.threads[0]!.id).catch(() => {});
  }

  // Dev-only: land on a specific view for UI verification.
  const startView = import.meta.env.VITE_START_VIEW as View | undefined;
  if (startView && ["chat", "shelves", "recipes", "settings"].includes(startView)) {
    app.view = startView;
    if (startView === "shelves" && import.meta.env.VITE_START_SHELF === "first") {
      app.openShelfId = app.shelves[0]?.id ?? null;
    }
  }

  events.engine((status) => (app.engine = status));
  if (app.settings?.activeModel) {
    api.warmEngine().catch(() => {});
  }
  events.download((download) => {
    const previous = app.downloads[download.id];
    app.downloads = {
      ...app.downloads,
      [download.id]: {
        ...previous,
        ...download,
        received: download.received ?? previous?.received ?? 0,
        total: download.total ?? previous?.total ?? null,
        phase: download.phase ?? previous?.phase ?? "downloading",
      },
    };
    if (download.done) {
      refreshSettings();
    }
    if (download.error && download.error !== "cancelled") {
      notify(
        download.error === "verification failed"
          ? "The download couldn't be verified. Try again."
          : download.error === "stalled"
            ? "The download stalled. Check your connection and try again."
            : "The download didn't finish. Try again.",
      );
    }
  });
  events.ingest(() => {
    app.ingestTick += 1;
  });
  events.shelfStats((event) => {
    const shelf = app.shelves.find((s) => s.id === event.shelfId);
    if (shelf) {
      shelf.stats = event.stats;
    }
  });
  events.shelves(() => refreshShelves());
  events.chat(handleChatEvent);
  events.update((update) => {
    app.update = update;
  });
  api
    .updateInfo()
    .then((update) => {
      if (update) app.update = update;
    })
    .catch(() => {});

  app.ready = true;
}
