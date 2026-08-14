// Typed surface over the Tauri commands and events.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ── Types (mirroring the Rust structs) ─────────────────────────────────

export interface PiiSummaryView {
  total: number;
  categories: Record<string, number>;
}

export interface ShelfStats {
  files: number;
  searchable: number;
  reading: number;
  errors: number;
  filesWithPii: number;
  pii: PiiSummaryView;
}

export interface LinkedView {
  sourceId: string;
  path: string;
  label: string;
}

export interface ShelfView {
  id: string;
  name: string;
  managedPath: string;
  linkedFolders: LinkedView[];
  stats: ShelfStats;
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
  body: string;
  path: string;
  score: number;
}

export interface ThreadMeta {
  id: string;
  title: string;
  shelfId?: string | null;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
}

export interface StoredMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
  thinking?: string | null;
  ts: string;
  shelfId?: string | null;
  sources: SourcePassage[];
  status: "done" | "stopped" | "error";
}

export interface EngineStatus {
  state: "no-model" | "downloading" | "stopped" | "starting" | "ready" | "error";
  detail?: string;
  modelName?: string;
}

export interface MachineProfile {
  totalRamBytes: number;
  availableRamBytes: number;
  cpu: string;
  appleSilicon: boolean;
  accelerator: string;
  freeDiskBytes: number;
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
  recommendationFits: boolean;
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
  shelfRoot: string;
  onboardingDone: boolean;
  activeModel?: ActiveModel | null;
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

// ── Commands ────────────────────────────────────────────────────────────

export const api = {
  shelvesList: () => invoke<ShelfView[]>("shelves_list"),
  shelfCreate: (name: string) => invoke<ShelfView>("shelf_create", { name }),
  shelfRemove: (shelfId: string) => invoke<void>("shelf_remove", { shelfId }),
  shelfAddLinked: (shelfId: string) => invoke<ShelfView | null>("shelf_add_linked", { shelfId }),
  shelfRemoveSource: (shelfId: string, sourceId: string) =>
    invoke<void>("shelf_remove_source", { shelfId, sourceId }),
  shelfImportPaths: (shelfId: string, paths: string[]) =>
    invoke<number>("shelf_import_paths", { shelfId, paths }),
  shelfImportDialog: (shelfId: string) => invoke<number>("shelf_import_dialog", { shelfId }),
  shelfDocuments: (shelfId: string) => invoke<DocumentMeta[]>("shelf_documents", { shelfId }),
  documentCard: (shelfId: string, docId: string) =>
    invoke<Card>("document_card", { shelfId, docId }),
  documentText: (shelfId: string, docId: string) =>
    invoke<string>("document_text", { shelfId, docId }),
  documentReprocess: (shelfId: string, docId: string) =>
    invoke<void>("document_reprocess", { shelfId, docId }),
  openOriginal: (path: string) => invoke<void>("open_original", { path }),
  revealItem: (path: string) => invoke<void>("reveal_item", { path }),

  threadsList: () => invoke<ThreadMeta[]>("threads_list"),
  threadCreate: (shelfId?: string | null) => invoke<ThreadMeta>("thread_create", { shelfId }),
  threadMessages: (threadId: string) => invoke<StoredMessage[]>("thread_messages", { threadId }),
  threadSetShelf: (threadId: string, shelfId?: string | null) =>
    invoke<void>("thread_set_shelf", { threadId, shelfId }),
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
  downloadCancel: (id: string) => invoke<void>("download_cancel", { id }),
  downloadSkipVerify: (id: string) => invoke<void>("download_skip_verify", { id }),

  settingsGet: () => invoke<SettingsView>("settings_get"),
  setHouseRules: (text: string) => invoke<void>("settings_set_house_rules", { text }),
  finishOnboarding: () => invoke<void>("settings_finish_onboarding"),
  resetWorkspace: (confirmation: string) =>
    invoke<void>("settings_reset_workspace", { confirmation }),
  redactText: (text: string) => invoke<string>("redact_text", { text }),
  textHasPii: (text: string) => invoke<boolean>("text_has_pii", { text }),
  diagnostics: () => invoke<Diagnostics>("diagnostics"),

  showAboutWindow: () => invoke<void>("show_about_window"),
  aboutInfo: () => invoke<AboutInfo>("about_info"),
  openExternal: (link: ExternalLink) => invoke<void>("open_external", { link }),

  updateInfo: () => invoke<AppUpdate | null>("update_info"),
  installUpdate: () => invoke<void>("install_update"),
  showUpdateWindow: () => invoke<void>("show_update_window"),

  recipesList: () => invoke<Recipe[]>("recipes_list"),
  recipeCreate: (name: string, prompt: string) => invoke<Recipe>("recipe_create", { name, prompt }),
  recipeDelete: (id: string) => invoke<void>("recipe_delete", { id }),
  recipesRestoreDefaults: () => invoke<Recipe[]>("recipes_restore_defaults"),
};

// ── Events ──────────────────────────────────────────────────────────────

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

export interface ChatEvent {
  threadId: string;
  messageId: string;
  kind: "queued" | "started" | "delta" | "thinking" | "promote" | "done" | "error";
  userMessageId?: string;
  text?: string;
  message?: StoredMessage;
  status?: string;
  error?: string;
}

export interface ShelfStatsEvent {
  shelfId: string;
  stats: ShelfStats;
}

export function onEvent<T>(name: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(name, (event) => handler(event.payload));
}

export const events = {
  engine: (h: (s: EngineStatus) => void) => onEvent("rebost://engine", h),
  download: (h: (d: DownloadEvent) => void) => onEvent("rebost://download", h),
  ingest: (h: (i: IngestEvent) => void) => onEvent("rebost://ingest", h),
  shelfStats: (h: (s: ShelfStatsEvent) => void) => onEvent("rebost://shelf-stats", h),
  shelves: (h: () => void) => onEvent("rebost://shelves", h),
  chat: (h: (c: ChatEvent) => void) => onEvent("rebost://chat", h),
  update: (h: (u: AppUpdate) => void) => onEvent("rebost://update", h),
  updateProgress: (h: (p: UpdateProgress) => void) => onEvent("rebost://update-progress", h),
};

// ── Formatting helpers ──────────────────────────────────────────────────

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
  phone: ["phone number", "phone numbers"],
  iban: ["IBAN", "IBANs"],
  nif: ["NIF / NIE", "NIF / NIE"],
  nie: ["NIE", "NIE"],
  credit_card: ["credit-card number", "credit-card numbers"],
  ip_address: ["IP address", "IP addresses"],
};

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
