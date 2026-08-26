/** Typed surface over the Tauri commands and events. */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Types mirroring the Rust structs. */

export interface PiiSummaryView {
  total: number;
  categories: Record<string, number>;
}

export interface ShelfStats {
  files: number;
  searchable: number;
  reading: number;
  errors: number;
  waiting: number;
  filesWithPii: number;
  pii: PiiSummaryView;
}

export interface LinkedView {
  sourceId: string;
  path: string;
  label: string;
  available?: boolean;
}

export type ThinkLevel = "off" | "light" | "deep";

export interface ShelfView {
  id: string;
  name: string;
  managedPath: string;
  linkedFolders: LinkedView[];
  stats: ShelfStats;
  threadId?: string | null;
  thinkLevel: ThinkLevel;
}

export interface DocumentMeta {
  id: string;
  shelfId: string;
  sourceId: string;
  sourceType: "imported" | "linked";
  path: string;
  relPath: string;
  fileName: string;
  format: string;
  sizeBytes: number;
  hash: string;
  status: "reading" | "ready" | "error";
  error?: string;
  passageCount: number;
  pages?: number;
  piiTotal: number;
  piiCategories?: Record<string, number>;
  ocr: boolean;
  updatedAt: string;
  sourceLabel: string;
}

export interface OutlineEntry {
  title: string;
  page?: number;
}

export interface Card {
  schema: string;
  id: string;
  source: "imported" | "linked";
  path: string;
  hash: string;
  title: string;
  format: string;
  language?: string;
  summary: string;
  keywords: string[];
  outline: OutlineEntry[];
  quality: "full" | "ocr";
  privacy: { total: number; categories?: Record<string, number> };
}

export interface SourcePassage {
  sid: string;
  documentId: string;
  shelfId: string;
  title: string;
  section?: string;
  pageStart?: number;
  pageEnd?: number;
  body?: string;
  path: string;
  score: number;
}

export interface DocumentTextWindow {
  text: string;
  startChar: number;
  endChar: number;
  totalChars: number;
  windowChars: number;
}

export interface ThreadMeta {
  id: string;
  title: string;
  shelfId?: string | null;
  uploadShelfId?: string | null;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
  avatarId: string;
}

export interface ChatActivityStep {
  stage: ChatPrepareStage;
  file?: string | null;
}

export interface StoredMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
  thinking?: string | null;
  activity?: ChatActivityStep[];
  ts: string;
  shelfId?: string | null;
  sources: SourcePassage[];
  status: "done" | "stopped" | "error";
}

export interface ThreadPage {
  messages: StoredMessage[];
  hasOlder: boolean;
}

export interface EngineStatus {
  state: "no-model" | "downloading" | "stopped" | "starting" | "ready" | "error";
  detail?: string;
  modelName?: string;
}

export interface MachineProfile {
  totalRamBytes: number;
  cpu: string;
  accelerator: string;
  freeDiskBytes: number;
  processArch: string;
  osArch: string;
}

export interface Recommendation {
  name: string;
  reference: string;
  provider: string;
  approxBytes: number;
  license: string;
  released: string;
  blurb: string;
}

export interface MachineView {
  profile: MachineProfile;
  recommendation: Recommendation;
  alternatives: Recommendation[];
  suggestions: Recommendation[];
}

export interface ActiveModel {
  file: string;
  name: string;
  source: string;
  reference: string;
  license?: string;
  sizeBytes: number;
}

export interface ModelSearchResult {
  id: string;
  name: string;
  source: string;
  reference: string;
  file?: string;
  sizeBytes?: number;
  license?: string;
  released?: string;
  downloads?: number;
  publisher?: string;
  official?: boolean;
  fits?: boolean;
}

export interface SettingsView {
  houseRules: string;
  onboardingDone: boolean;
  activeModel?: ActiveModel | null;
  allowOnlineResearch: boolean;
}

export interface ImportResult {
  queued: number;
  names: string[];
  cancelled: boolean;
  atLimit: boolean;
  skippedLong?: number;
}

export interface AddLinkedResult {
  shelf: ShelfView;
  queued: number;
  atLimit: boolean;
}

export interface Recipe {
  id: string;
  name: string;
  prompt: string;
  builtin: boolean;
}

export type ExternalLink = "repository";

export interface AboutInfo {
  version: string;
  repositoryUrl: string;
}

export interface AppUpdate {
  version: string;
  currentVersion: string;
  notes?: string | null;
}

export type UpdateProgress =
  | { event: "started"; data: { contentLength?: number | null } }
  | { event: "progress"; data: { chunkLength: number } }
  | { event: "finished"; data?: null };

export interface Diagnostics {
  version: string;
  dataDir: string;
  engineBuild: string;
  engineState: EngineStatus;
  model?: ActiveModel | null;
  indexRecords: number;
  contextBudgetChars: number;
  benchmark?: {
    promptTokensPerSecond: number;
    generationTokensPerSecond: number;
    measuredAt: string;
    modelFile: string;
  } | null;
  machine: MachineProfile;
  engineLogPath: string;
  engineLogPresent: boolean;
  supportedFormats: string[];
}

/** Turn a Tauri or JS failure into a string the UI can show or log. */
export function invokeError(error: unknown): string {
  if (typeof error === "string" && error.trim()) return error;
  if (error instanceof Error && error.message.trim()) return error.message;
  return String(error);
}

const USER_ERROR_FALLBACK = "Something went wrong. Try again.";

const BANNED_ERROR_TERMS = [
  "gguf",
  "llama",
  "sha-256",
  "sha256",
  "vulkan",
  "tantivy",
  "ocr",
  "tessdata",
  "loopback",
  "llama-server",
  "aarch64",
  "x86_64",
  "tok/s",
  "checksum",
] as const;

/** Two-beat toast copy. Pins stay behind Diagnostics. */
export function userFacingError(error: unknown): string {
  const trimmed = invokeError(error).trim();
  if (!trimmed) return USER_ERROR_FALLBACK;
  const lower = trimmed.toLowerCase();

  if (lower.includes("invalid id")) return "That request was not valid.";
  if (lower.includes("not in a shelf") || lower.includes("not allowed")) {
    return "That file is not in a Shelf Rebost knows.";
  }
  if (
    lower.includes("shelf not found") ||
    lower.includes("file not found") ||
    lower.includes("thread not found") ||
    lower.includes("recipe not found")
  ) {
    return "That item is no longer available.";
  }
  if (
    lower.includes("no ai model") ||
    lower.includes("no model installed") ||
    lower.includes("model file missing")
  ) {
    return "Rebost needs an AI before it can answer.";
  }
  if (lower.includes("switch-failed")) {
    return "That AI didn't start. You're still using the previous one.";
  }
  if (lower.includes("warmup-failed")) {
    return "That AI didn't start. Try again, or pick a smaller one.";
  }
  if (lower.includes("incompatible-format")) {
    return "This AI uses a format Rebost can't run. Pick another.";
  }
  if (
    lower.includes("llama-server") ||
    lower.includes("health timeout") ||
    lower.includes("no free port") ||
    lower.includes("engine archive") ||
    lower.includes("engine binary")
  ) {
    return "Rebost isn't ready yet. Try again in a moment.";
  }
  if (
    lower.includes("verification failed") ||
    lower.includes("sha-256") ||
    lower.includes("sha256") ||
    lower.includes("checksum")
  ) {
    return "The download couldn't be verified. Try again.";
  }
  if (
    lower.includes("generation failed") ||
    lower.includes("generation stalled") ||
    lower.includes("empty generation") ||
    lower.includes("chat stream")
  ) {
    return "Rebost couldn't finish that answer. Try again.";
  }
  if (lower.includes("stalled")) {
    return "The download stalled. Check your connection and try again.";
  }
  if (lower.includes("rate-limited") || lower.includes("rate limited")) {
    return "The download was rate-limited. Wait a moment and try again.";
  }
  if (
    lower.includes("download failed") ||
    lower.includes("incomplete range") ||
    lower.includes("range wrote") ||
    lower.includes("server ignored range")
  ) {
    return "The download didn't finish. Try again.";
  }
  if (
    lower.includes(".gguf") ||
    lower.includes("gguf") ||
    lower.includes("invalid model") ||
    lower.includes("unsupported model") ||
    lower.includes("no usable model") ||
    lower.includes("no model layer")
  ) {
    return "That AI isn't available. Try another.";
  }
  if (lower.includes("couldn't read any text")) {
    return "Rebost couldn't read any text in this file.";
  }
  if (lower.includes("unsupported format")) {
    return "This file type isn't supported.";
  }
  if (lower.includes("invalid file name")) {
    return "That file couldn't be added. Try again.";
  }

  if (alreadyQuietError(trimmed, lower)) return trimmed;
  return USER_ERROR_FALLBACK;
}

function alreadyQuietError(text: string, lower: string): boolean {
  if ([...text].length > 180) return false;
  if (BANNED_ERROR_TERMS.some((pin) => lower.includes(pin))) return false;
  if (lower.includes("::") || lower.includes(".rs") || lower.includes("anyhow")) return false;
  return text.endsWith(".") || text.endsWith("?");
}

// Commands

export const api = {
  shelvesList: () => invoke<ShelfView[]>("shelves_list"),
  shelfGet: (shelfId: string) => invoke<ShelfView>("shelf_get", { shelfId }),
  shelfCreate: (name: string) => invoke<ShelfView>("shelf_create", { name }),
  shelfRename: (shelfId: string, name: string) =>
    invoke<ShelfView>("shelf_rename", { shelfId, name }),
  shelfRemove: (shelfId: string) => invoke<void>("shelf_remove", { shelfId }),
  shelfSetThinkLevel: (shelfId: string, thinkLevel: ThinkLevel) =>
    invoke<ShelfView>("shelf_set_think_level", { shelfId, thinkLevel }),
  shelfAddLinked: (shelfId: string) =>
    invoke<AddLinkedResult | null>("shelf_add_linked", { shelfId }),
  shelfRemoveSource: (shelfId: string, source: { sourceId?: string; path?: string }) => {
    const payload: Record<string, string> = { shelfId };
    if (source.sourceId) payload.sourceId = source.sourceId;
    if (source.path) payload.path = source.path;
    return invoke<void>("shelf_remove_source", payload);
  },
  shelfImportPaths: (shelfId: string, paths: string[]) =>
    invoke<ImportResult>("shelf_import_paths", { shelfId, paths }),
  shelfImportDialog: (shelfId: string) => invoke<ImportResult>("shelf_import_dialog", { shelfId }),
  pickFiles: () => invoke<string[] | null>("pick_files"),
  shelfDocuments: (shelfId: string) => invoke<DocumentMeta[]>("shelf_documents", { shelfId }),
  documentCard: (shelfId: string, docId: string) =>
    invoke<Card>("document_card", { shelfId, docId }),
  documentText: (
    shelfId: string,
    docId: string,
    opts?: { startChar?: number; page?: number; section?: string; around?: string },
  ) =>
    invoke<DocumentTextWindow>("document_text", {
      shelfId,
      docId,
      startChar: opts?.startChar ?? null,
      page: opts?.page ?? null,
      section: opts?.section ?? null,
      around: opts?.around ?? null,
    }),
  documentReprocess: (shelfId: string, docId: string) =>
    invoke<void>("document_reprocess", { shelfId, docId }),
  shelfRetryFailed: (shelfId: string) => invoke<number>("shelf_retry_failed", { shelfId }),
  openOriginal: (path: string) => invoke<void>("open_original", { path }),
  revealItem: (path: string) => invoke<void>("reveal_item", { path }),

  threadsList: () => invoke<ThreadMeta[]>("threads_list"),
  threadCreate: (shelfId?: string | null) => invoke<ThreadMeta>("thread_create", { shelfId }),
  threadMessages: (threadId: string, beforeId?: string | null) =>
    invoke<ThreadPage>("thread_messages", { threadId, beforeId: beforeId ?? null }),
  threadSetShelf: (threadId: string, shelfId?: string | null) =>
    invoke<void>("thread_set_shelf", { threadId, shelfId }),
  threadRename: (threadId: string, title: string) =>
    invoke<void>("thread_rename", { threadId, title }),
  threadExport: (threadId: string) => invoke<boolean>("thread_export", { threadId }),
  threadEnsureUploadShelf: (threadId: string) =>
    invoke<ShelfView>("thread_ensure_upload_shelf", { threadId }),
  threadDelete: (threadId: string) => invoke<void>("thread_delete", { threadId }),
  chatSend: (threadId: string, text: string, shelfId?: string | null) =>
    invoke<void>("chat_send", { threadId, text, shelfId }),
  chatCancel: (messageId: string) => invoke<void>("chat_cancel", { messageId }),
  warmEngine: () => invoke<void>("warm_engine"),

  engineStatus: () => invoke<EngineStatus>("engine_status"),
  machineProfile: () => invoke<MachineView>("machine_profile"),
  modelsSearch: (query: string) => invoke<ModelSearchResult[]>("models_search", { query }),
  modelInstall: (source: string, reference: string, name: string, license?: string) =>
    invoke<void>("model_install", { source, reference, name, license }),
  openModelPage: (source: string, reference: string) =>
    invoke<void>("open_model_page", { source, reference }),
  downloadCancel: (id: string) => invoke<void>("download_cancel", { id }),
  downloadSkipVerify: (id: string) => invoke<void>("download_skip_verify", { id }),

  settingsGet: () => invoke<SettingsView>("settings_get"),
  setHouseRules: (text: string) => invoke<void>("settings_set_house_rules", { text }),
  setAllowOnlineResearch: (enabled: boolean) =>
    invoke<void>("settings_set_allow_online_research", { enabled }),
  finishOnboarding: () => invoke<void>("settings_finish_onboarding"),
  resetWorkspace: (confirmation: string) =>
    invoke<void>("settings_reset_workspace", { confirmation }),
  redactText: (text: string) => invoke<string>("redact_text", { text }),
  textHasPii: (text: string) => invoke<boolean>("text_has_pii", { text }),
  diagnostics: () => invoke<Diagnostics>("diagnostics"),
  openEngineLog: () => invoke<void>("open_engine_log"),

  showAboutWindow: () => invoke<void>("show_about_window"),
  devSnapshot: (path: string, label?: string) =>
    invoke<void>("dev_snapshot", { path, label: label ?? null }),
  aboutInfo: () => invoke<AboutInfo>("about_info"),
  openExternal: (link: ExternalLink) => invoke<void>("open_external", { link }),

  updateInfo: () => invoke<AppUpdate | null>("update_info"),
  installUpdate: () => invoke<void>("install_update"),
  showUpdateWindow: () => invoke<void>("show_update_window"),

  recipesList: () => invoke<Recipe[]>("recipes_list"),
  recipeCreate: (name: string, prompt: string) => invoke<Recipe>("recipe_create", { name, prompt }),
  recipeUpdate: (id: string, name: string, prompt: string) =>
    invoke<Recipe>("recipe_update", { id, name, prompt }),
  recipeDelete: (id: string) => invoke<void>("recipe_delete", { id }),
  recipesRestoreDefaults: () => invoke<Recipe[]>("recipes_restore_defaults"),
};

// Events

export interface IngestEvent {
  shelfId: string;
  documentId: string;
  fileName?: string;
  status: "reading" | "ready" | "error" | "removed";
  error?: string;
  piiTotal?: number;
  passages?: number;
}

export type DownloadPhase = "downloading" | "verifying";

export interface DownloadEvent {
  kind: "engine" | "model";
  id: string;
  name: string;
  received?: number;
  total?: number | null;
  done: boolean;
  error?: string | null;
  phase?: DownloadPhase;
}

export type ChatPrepareStage =
  "waiting" | "looking" | "reading" | "opening" | "around" | "chats" | "web" | "page" | "thinking";

export interface ChatEvent {
  threadId: string;
  messageId: string;
  kind:
    "queued" | "started" | "delta" | "thinking" | "promote" | "done" | "error" | "status" | "clear";
  userMessageId?: string;
  text?: string;
  message?: StoredMessage;
  status?: string;
  error?: string;
  stage?: ChatPrepareStage;
  file?: string;
}

export interface ShelfStatsEvent {
  shelfId: string;
  stats: ShelfStats;
}

export function onEvent<T>(name: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(name, (event) => handler(event.payload));
}

export type MenuAction =
  "new-conversation" | "view-chat" | "view-shelves" | "view-recipes" | "view-settings";

export interface MenuEvent {
  action: MenuAction;
}

export const events = {
  engine: (h: (s: EngineStatus) => void) => onEvent("rebost://engine", h),
  download: (h: (d: DownloadEvent) => void) => onEvent("rebost://download", h),
  ingest: (h: (i: IngestEvent) => void) => onEvent("rebost://ingest", h),
  shelfStats: (h: (s: ShelfStatsEvent) => void) => onEvent("rebost://shelf-stats", h),
  shelves: (h: () => void) => onEvent("rebost://shelves", h),
  chat: (h: (c: ChatEvent) => void) => onEvent("rebost://chat", h),
  menu: (h: (m: MenuEvent) => void) => onEvent("rebost://menu", h),
  update: (h: (u: AppUpdate) => void) => onEvent("rebost://update", h),
  updateProgress: (h: (p: UpdateProgress) => void) => onEvent("rebost://update-progress", h),
};

// Formatting helpers

export function formatBytes(bytes?: number | null): string {
  return formatByteAmount(bytes, false);
}

/** One decimal for GB so a transfer cannot look finished while it is still moving. */
export function formatTransferBytes(bytes?: number | null): string {
  return formatByteAmount(bytes, true);
}

function formatByteAmount(bytes: number | null | undefined, transfer: boolean): string {
  if (bytes == null) return "—";
  if (bytes <= 0) return transfer ? "0 GB" : "—";
  const gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 1) {
    const digits = transfer || gb < 10 ? 1 : 0;
    return `${gb.toFixed(digits)} GB`;
  }
  const mb = bytes / (1024 * 1024);
  if (mb >= 1) return `${mb.toFixed(transfer ? 1 : 0)} MB`;
  return `${(bytes / 1024).toFixed(0)} KB`;
}

/** Compact download counts from Hugging Face (`1200` → `1.2k`). */
export function formatCount(n?: number | null): string {
  if (n == null || n < 0) return "—";
  if (n < 1000) return String(n);
  const compact = (value: number, suffix: string) => {
    const digits = value >= 10 ? 0 : 1;
    return `${value.toFixed(digits).replace(/\.0$/, "")}${suffix}`;
  };
  if (n < 1_000_000) return compact(n / 1000, "k");
  return compact(n / 1_000_000, "M");
}

export function downloadHeadline(download: DownloadEvent): string {
  const phase: DownloadPhase = download.phase ?? "downloading";
  switch (phase) {
    case "verifying":
      return "Checking the download…";
    case "downloading":
      return download.kind === "engine" ? "Preparing the AI…" : `Downloading ${download.name}…`;
    default: {
      const _exhaustive: never = phase;
      return _exhaustive;
    }
  }
}

/** Toast for a failed model or engine download. Null when the user cancelled. */
export function downloadErrorMessage(error: string): string | null {
  if (error === "cancelled") return null;
  switch (error) {
    case "verification failed":
      return "The download couldn't be verified. Try again.";
    case "stalled":
      return "The download stalled. Check your connection and try again.";
    case "switch-failed":
      return "That AI didn't start. You're still using the previous one.";
    case "warmup-failed":
      return "That AI didn't start. Try again, or pick a smaller one.";
    case "incompatible-format":
      return "This AI uses a format Rebost can't run. Pick another.";
    default:
      return "The download didn't finish. Try again.";
  }
}

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
] as const;

/** Catalog the AI is downloaded from. Hugging Face wins when both list it. */
export function catalogHostLabel(source: string): string {
  switch (source) {
    case "huggingface":
    case "huggingface+ollama":
      return "Hugging Face";
    case "ollama":
      return "Ollama";
    default:
      return "the catalog";
  }
}

/** Catalog dates are `YYYY-MM`. */
export function formatReleased(ym: string): string {
  const [year, month] = ym.split("-");
  const idx = Number(month) - 1;
  if (!year || Number.isNaN(idx) || idx < 0 || idx > 11) return ym;
  return `${MONTHS[idx]} ${year}`;
}

export function formatWhen(iso: string): string {
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return "—";
  const now = new Date();
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(now) - startOfDay(then)) / 86_400_000);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  return then.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

export const PII_LABELS: Record<string, [string, string]> = {
  email: ["email", "emails"],
  name: ["name", "names"],
  ssn: ["Social Security number", "Social Security numbers"],
  phone: ["phone number", "phone numbers"],
  iban: ["IBAN", "IBANs"],
  nif: ["NIF / NIE", "NIF / NIE"],
  nie: ["NIE", "NIE"],
  credit_card: ["credit-card number", "credit-card numbers"],
  ip_address: ["IP address", "IP addresses"],
};

export const PII_CATEGORY_ORDER = [
  "email",
  "name",
  "ssn",
  "phone",
  "nif",
  "nie",
  "iban",
  "credit_card",
  "ip_address",
] as const;

/** Empty Privacy Lens: name the categories; do not call the file clean. */
export const PII_EMPTY_HINT =
  "No emails, phone numbers, IBANs, tax ids, Social Security numbers, or labeled names in this file.";

export function piiLabel(category: string, count: number): string {
  const pair = PII_LABELS[category] ?? [category, category];
  return count === 1 ? pair[0] : pair[1];
}

export function fileTypeLabel(doc: DocumentMeta): string {
  const format = doc.format.toUpperCase();
  if (
    doc.pages &&
    doc.pages > 0 &&
    ["PDF", "DOCX", "DOC", "PPTX", "PPT", "ODT", "ODP"].includes(format)
  ) {
    return `${format} · ${doc.pages}p`;
  }
  return format;
}
