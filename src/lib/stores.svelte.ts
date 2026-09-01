/** Global app state (Svelte 5 runes) fed by backend events. */
import {
  api,
  downloadErrorMessage,
  events,
  invokeError,
  userFacingError,
  type AppUpdate,
  type ChatEvent,
  type DownloadEvent,
  type EngineStatus,
  type MenuAction,
  type SettingsView,
  type ShelfView,
  type TextSize,
  type StoredMessage,
  type ThinkLevel,
  type ThreadMeta,
} from "./api";
import { reduceChatEvent, type ChatPendingMap } from "./chat-reducer";
import {
  loadPreferredShelf,
  savePreferredShelf,
  shelfForNewConversation,
  type ShelfPreference,
} from "./shelf-preference";
import { clipChars, PROMPT_MAX_CHARS } from "./text-cap";
import { applyTextSize, parseTextSize, persistTextSize, stepTextSize } from "./text-size";
import { applyLocale, parseAppLocale, parseLocalePref, type LocalePref } from "./i18n.svelte";

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
  /** Shelf open on the Shelves view. */
  openShelfId: null as string | null,
  /** Open the new-shelf form when the Shelves view next mounts. */
  createShelf: false,
  /** Bumped so shelf views refetch documents. */
  ingestTick: 0,
  /** Set only when a newer GitHub release is confirmed. Never set on a failed check. */
  update: null as AppUpdate | null,
});

export const chatState = $state({
  activeThreadId: null as string | null,
  messages: [] as StoredMessage[],
  /** Composer draft — lives here so Recipes can pre-fill it. */
  draft: "",
  /** Bumped when a Recipe lands a prompt, so the composer grabs focus. */
  draftFocus: 0,
  /** Shelf the composer shows; synced from the open thread when there is one. */
  selectedShelfId: null as string | null,
  /** Hidden upload Shelf for the open conversation, if files were attached. */
  uploadShelf: null as ShelfView | null,
  /** In-flight assistant message per conversation. */
  pending: {} as ChatPendingMap,
  /** Threads that have sent but not yet received `queued`. */
  outbound: {} as Record<string, boolean>,
  /** Stop pressed before the backend had a message id to cancel. */
  cancelWhenQueued: {} as Record<string, boolean>,
  /** Older messages exist above the open window. */
  hasOlder: false,
  loadingOlder: false,
});

let creatingConversation = false;
let preferredShelfId: ShelfPreference = loadPreferredShelf();

/** Remember a manual composer pick, including No Shelf. */
export function rememberPreferredShelf(shelfId: string | null) {
  preferredShelfId = shelfId;
  savePreferredShelf(shelfId);
}

/** Shelf for a new conversation: last manual pick, else the only Shelf. */
export function preferredShelfForNew(): string | null {
  return shelfForNewConversation(
    preferredShelfId,
    app.shelves.map((shelf) => shelf.id),
  );
}

export function fillDraft(text: string) {
  chatState.draft = clipChars(text, PROMPT_MAX_CHARS);
  chatState.draftFocus += 1;
}

function clearConversation() {
  chatState.activeThreadId = null;
  chatState.messages = [];
  chatState.uploadShelf = null;
  chatState.hasOlder = false;
  chatState.loadingOlder = false;
}

/** Empty Chat (screenshot runner, or leaving a thread). */
export function closeConversation() {
  clearConversation();
  chatState.draft = "";
  chatState.selectedShelfId = preferredShelfForNew();
}

/** Start a fresh conversation with a Recipe prompt already in the composer. */
export function startRecipe(prompt: string) {
  clearConversation();
  chatState.selectedShelfId = preferredShelfForNew();
  fillDraft(prompt);
  app.view = "chat";
}

/** Same as the Chat sidebar +: a new empty conversation. */
export async function newConversation() {
  if (creatingConversation || !app.ready || app.onboarding) return;
  if (chatState.outbound["new"]) return;
  creatingConversation = true;
  try {
    app.view = "chat";
    clearConversation();
    chatState.draft = "";
    const shelfId = preferredShelfForNew();
    chatState.selectedShelfId = shelfId;
    const thread = await api.threadCreate(shelfId);
    await refreshThreads();
    await openThread(thread.id);
  } catch (error) {
    notifyInvokeError(error);
  } finally {
    creatingConversation = false;
  }
}

export function goView(view: View) {
  if (!app.ready || app.onboarding) return;
  app.view = view;
  if (view === "shelves") app.openShelfId = null;
}

export function handleMenuAction(action: MenuAction) {
  switch (action) {
    case "new-conversation":
      void newConversation();
      return;
    case "view-chat":
      goView("chat");
      return;
    case "view-shelves":
      goView("shelves");
      return;
    case "view-recipes":
      goView("recipes");
      return;
    case "view-settings":
      goView("settings");
      return;
    case "text-larger":
      bumpTextSize(1);
      return;
    case "text-smaller":
      bumpTextSize(-1);
      return;
    default: {
      const _exhaustive: never = action;
      return _exhaustive;
    }
  }
}

export async function refreshShelves() {
  app.shelves = await api.shelvesList();
}

export async function setShelfThinkLevel(shelfId: string, level: ThinkLevel) {
  const previous = app.shelves.find((shelf) => shelf.id === shelfId)?.thinkLevel ?? "off";
  if (previous === level) return;
  app.shelves = app.shelves.map((shelf) =>
    shelf.id === shelfId ? { ...shelf, thinkLevel: level } : shelf,
  );
  try {
    const updated = await api.shelfSetThinkLevel(shelfId, level);
    app.shelves = app.shelves.map((shelf) => (shelf.id === shelfId ? updated : shelf));
  } catch (error) {
    app.shelves = app.shelves.map((shelf) =>
      shelf.id === shelfId ? { ...shelf, thinkLevel: previous } : shelf,
    );
    notifyInvokeError(error);
  }
}

export async function refreshThreads() {
  app.threads = await api.threadsList();
}

export async function refreshSettings() {
  app.settings = await api.settingsGet();
  paintTextSize(parseTextSize(app.settings.textSize));
  applyLocale(parseAppLocale(app.settings.resolvedLocale));
}

function paintTextSize(size: TextSize) {
  applyTextSize(size);
  persistTextSize(size);
}

let textSizeBump = 0;

function bumpTextSize(delta: 1 | -1) {
  const generation = ++textSizeBump;
  queueMicrotask(() => {
    if (generation !== textSizeBump) return;
    const next = stepTextSize(parseTextSize(app.settings?.textSize), delta);
    if (next === parseTextSize(app.settings?.textSize)) return;
    void setTextSize(next);
  });
}

export async function setTextSize(next: TextSize) {
  const previous = parseTextSize(app.settings?.textSize);
  if (next === previous && app.settings) return;
  paintTextSize(next);
  if (app.settings) app.settings = { ...app.settings, textSize: next };
  try {
    await api.setTextSize(next);
    await refreshSettings();
  } catch (error) {
    paintTextSize(previous);
    if (app.settings) app.settings = { ...app.settings, textSize: previous };
    notifyInvokeError(error);
  }
}

export async function setUiLocale(next: LocalePref) {
  const previousPref = parseLocalePref(app.settings?.uiLocale);
  const previousResolved = parseAppLocale(app.settings?.resolvedLocale);
  if (next === previousPref && app.settings) return;
  if (app.settings) app.settings = { ...app.settings, uiLocale: next };
  try {
    const view = await api.setUiLocale(next);
    app.settings = view;
    applyLocale(parseAppLocale(view.resolvedLocale));
  } catch (error) {
    if (app.settings) app.settings = { ...app.settings, uiLocale: previousPref };
    applyLocale(previousResolved);
    notifyInvokeError(error);
  }
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
  chatState.hasOlder = false;
  chatState.loadingOlder = false;
  const page = await api.threadMessages(threadId);
  if (chatState.activeThreadId !== threadId) return;
  chatState.messages = page.messages;
  chatState.hasOlder = page.hasOlder;
  const thread = app.threads.find((item) => item.id === threadId);
  chatState.uploadShelf = null;
  if (thread?.uploadShelfId) {
    try {
      const shelf = await api.shelfGet(thread.uploadShelfId);
      if (chatState.activeThreadId === threadId) {
        chatState.uploadShelf = shelf;
      }
    } catch (error) {
      logInvokeError(error, "load uploaded files");
    }
  }
}

/** Prepend the next older window of the open conversation. */
export async function loadOlderMessages() {
  const threadId = chatState.activeThreadId;
  const first = chatState.messages[0];
  if (!threadId || !first || !chatState.hasOlder || chatState.loadingOlder) return;
  chatState.loadingOlder = true;
  try {
    const page = await api.threadMessages(threadId, first.id);
    if (chatState.activeThreadId !== threadId) return;
    chatState.messages = [...page.messages, ...chatState.messages];
    chatState.hasOlder = page.hasOlder;
  } finally {
    if (chatState.activeThreadId === threadId) {
      chatState.loadingOlder = false;
    }
  }
}

export function openCreateShelf() {
  app.openShelfId = null;
  app.createShelf = true;
  app.view = "shelves";
}

function dropFlag(map: Record<string, boolean>, key: string): Record<string, boolean> {
  if (!map[key]) return map;
  const next = { ...map };
  delete next[key];
  return next;
}

function handleChatEvent(event: ChatEvent) {
  if (event.kind === "queued" && chatState.cancelWhenQueued[event.threadId]) {
    api.chatCancel(event.messageId).catch((error) => logInvokeError(error, "cancel queued"));
  }
  if (event.kind === "queued" || event.kind === "done" || event.kind === "error") {
    chatState.outbound = dropFlag(chatState.outbound, event.threadId);
    chatState.cancelWhenQueued = dropFlag(chatState.cancelWhenQueued, event.threadId);
  }
  const result = reduceChatEvent(chatState.pending, event);
  chatState.pending = result.pending;
  if (result.append && chatState.activeThreadId === event.threadId) {
    chatState.messages = [...chatState.messages, result.append];
  }
  if (result.refreshThreads) {
    refreshThreads();
  }
  if (result.error) {
    notify(userFacingError(result.error));
  }
}

let notifier: (message: string) => void = () => {};
export function setNotifier(fn: (message: string) => void) {
  notifier = fn;
}
export function notify(message: string) {
  notifier(message);
}

/** Log a failed invoke and show it in the toast. */
export function notifyInvokeError(error: unknown) {
  const raw = invokeError(error);
  console.error(raw);
  notify(userFacingError(error));
}

/** Log a failed invoke that should not interrupt the user (warmup, update check). */
export function logInvokeError(error: unknown, context: string) {
  console.error(`${context}: ${invokeError(error)}`);
}

/** Wire events + initial data. Called once from App.svelte. */
export async function bootstrap() {
  await Promise.all([refreshShelves(), refreshThreads(), refreshSettings()]);
  const engineStatus = await api.engineStatus();
  app.engine = engineStatus;
  app.onboarding = !(app.settings?.onboardingDone || app.settings?.activeModel);

  // Chat is home: reopen the most recent conversation, or a numbered one
  // for screenshots (`VITE_START_THREAD=1` is the top of the list).
  if (app.threads.length > 0) {
    const startThread = Number(import.meta.env.VITE_START_THREAD ?? "0");
    const index = startThread >= 1 ? startThread - 1 : 0;
    const thread = app.threads[index] ?? app.threads[0];
    if (thread) await openThread(thread.id);
  } else {
    chatState.selectedShelfId = preferredShelfForNew();
  }

  // Dev-only: land on a specific view for UI verification.
  const startView = import.meta.env.VITE_START_VIEW as View | undefined;
  if (startView && ["chat", "shelves", "recipes", "settings"].includes(startView)) {
    app.view = startView;
    if (startView === "shelves" && import.meta.env.VITE_START_SHELF === "first") {
      app.openShelfId = app.shelves[0]?.id ?? null;
    }
  }

  events.engine((status) => {
    const previousName = app.engine.modelName;
    app.engine = status;
    if (status.modelName && status.modelName !== previousName) {
      void refreshSettings();
    }
  });
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
    if (download.error) {
      const message = downloadErrorMessage(download.error);
      if (message) notify(message);
    }
  });
  let ingestQueued = false;
  events.ingest(() => {
    if (ingestQueued) return;
    ingestQueued = true;
    setTimeout(() => {
      ingestQueued = false;
      app.ingestTick += 1;
    }, 400);
  });
  events.shelfStats((event) => {
    const shelf = app.shelves.find((s) => s.id === event.shelfId);
    if (shelf) {
      shelf.stats = { ...event.stats, waiting: event.stats.waiting ?? 0 };
    }
    if (chatState.uploadShelf?.id === event.shelfId) {
      chatState.uploadShelf = {
        ...chatState.uploadShelf,
        stats: { ...event.stats, waiting: event.stats.waiting ?? 0 },
      };
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
    .catch((error) => logInvokeError(error, "update check"));

  app.ready = true;
}
