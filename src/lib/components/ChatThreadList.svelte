<script lang="ts">
  import { api, type ShelfView, type ThreadMeta, type ThreadSearchHit } from "$lib/api";
  import { t } from "$lib/i18n.svelte";
  import { threadShelfSubtitle } from "$lib/shelf-label";
  import { popupNativeMenu, type NativeMenuEntry } from "$lib/native-menu";
  import { isMac, isWindows } from "$lib/platform";
  import {
    LatestRequest,
    SEARCH_DEBOUNCE_MS,
    isListFindShortcut,
    searchGroups,
    searchKeyAction,
  } from "$lib/thread-search";
  import { Plus, Trash2, Lock, Pencil, Search, X } from "@lucide/svelte";
  import ConversationFace from "./ConversationFace.svelte";
  import ChatThreadSearchResults from "./ChatThreadSearchResults.svelte";

  let {
    threads,
    activeThreadId,
    shelves,
    onOpen,
    onNew,
    onRemove,
    onRename,
    onExport,
    onClose,
    onLeaveSearch,
  }: {
    threads: ThreadMeta[];
    activeThreadId: string | null;
    shelves: ShelfView[];
    onOpen: (id: string) => void;
    onNew: () => void;
    onRemove: (id: string) => void;
    onRename: (id: string, title: string) => void;
    onExport: (id: string) => void;
    onClose?: () => void;
    /** Escape in an empty search field: hand focus back to the composer. */
    onLeaveSearch?: () => void;
  } = $props();

  const open = $derived(threads.length > 0);
  // macOS and Windows: a full-height column beside the sidebar, its header
  // level with Chat's macOS toolbar. The narrow-window drawer keeps the card.
  const column = $derived((isMac() || isWindows()) && !onClose);
  let editingId = $state<string | null>(null);
  let editTitle = $state("");

  function beginRename(thread: ThreadMeta, event?: Event) {
    event?.stopPropagation();
    editingId = thread.id;
    editTitle = thread.title;
  }

  function commitRename(threadId: string) {
    const title = editTitle.trim();
    editingId = null;
    if (!title || title === threads.find((t) => t.id === threadId)?.title) return;
    onRename(threadId, title);
  }

  function onRenameKey(event: KeyboardEvent, threadId: string) {
    if (event.key === "Enter") {
      event.preventDefault();
      commitRename(threadId);
    } else if (event.key === "Escape") {
      event.preventDefault();
      editingId = null;
    }
  }

  // Search: titles filter on the client at once; message matches come from
  // the index after a short pause, and only the latest request may land.
  const uid = $props.id();
  const listboxId = `${uid}-results`;
  let searchInput = $state<HTMLInputElement | null>(null);
  let query = $state("");
  let hits = $state<ThreadSearchHit[]>([]);
  let hitsFor = $state<string | null>(null);
  let activeIndex = $state(-1);
  const requests = new LatestRequest();
  let debounce: ReturnType<typeof setTimeout> | undefined;

  const trimmed = $derived(query.trim());
  const groups = $derived(searchGroups(threads, trimmed, hits));
  const rows = $derived([...groups.titles, ...groups.messages]);
  const active = $derived(activeIndex < rows.length ? activeIndex : -1);
  const settled = $derived(trimmed !== "" && hitsFor === trimmed);
  const announcement = $derived.by(() => {
    if (!settled) return "";
    if (rows.length === 0) return t("chat.searchNoResults", { query: trimmed });
    return rows.length === 1
      ? t("chat.oneConversationFound")
      : t("chat.manyConversationsFound", { count: rows.length });
  });

  $effect(() => () => clearTimeout(debounce));

  export function focusSearch() {
    searchInput?.focus();
    searchInput?.select();
  }

  function onQueryInput() {
    activeIndex = -1;
    clearTimeout(debounce);
    const text = query.trim();
    if (!text) {
      requests.cancel();
      hits = [];
      hitsFor = null;
      return;
    }
    const request = requests.begin();
    debounce = setTimeout(() => {
      api
        .threadsSearch(text)
        .catch(() => [] as ThreadSearchHit[])
        .then((result) => {
          if (!requests.isCurrent(request)) return;
          hits = result;
          hitsFor = text;
        });
    }, SEARCH_DEBOUNCE_MS);
  }

  function clearSearch() {
    query = "";
    onQueryInput();
    searchInput?.focus();
  }

  function onSearchKey(event: KeyboardEvent) {
    if (event.isComposing) return;
    const action = searchKeyAction(event.key, query, active, rows.length);
    if (!action) return;
    // Also keeps the narrow-window drawer open on the clearing Escape.
    event.preventDefault();
    event.stopPropagation();
    switch (action.kind) {
      case "move":
        activeIndex = action.index;
        return;
      case "open": {
        const row = rows[action.index];
        if (row) onOpen(row.thread.id);
        return;
      }
      case "clear":
        clearSearch();
        return;
      case "leave":
        onLeaveSearch?.();
        return;
    }
  }

  function onListKey(event: KeyboardEvent) {
    if (!isListFindShortcut(event)) return;
    event.preventDefault();
    focusSearch();
  }

  function onThreadMenu(thread: ThreadMeta, event: MouseEvent) {
    if (editingId === thread.id) return;
    event.preventDefault();
    event.stopPropagation();
    const entries: NativeMenuEntry[] = [
      { kind: "item", text: t("chat.rename"), action: () => beginRename(thread) },
    ];
    if (thread.messageCount > 0) {
      entries.push({
        kind: "item",
        text: t("chat.download"),
        action: () => onExport(thread.id),
      });
    }
    entries.push(
      { kind: "separator" },
      { kind: "item", text: t("chat.delete"), action: () => onRemove(thread.id) },
    );
    void popupNativeMenu(entries).catch(() => {});
  }
</script>

<div
  class="flex h-full min-h-0 shrink-0 overflow-hidden transition-[width] duration-300 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none {open
    ? 'w-[16.5rem]'
    : 'w-0'}"
  aria-hidden={!open}
>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions (⌘F / Ctrl+F from anywhere in the list) -->
  <aside
    onkeydown={onListKey}
    class="flex shrink-0 flex-col overflow-hidden transition-transform duration-300 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none {column
      ? 'w-[16.5rem] border-r border-paper-line bg-surface'
      : 'mt-1 mr-3 mb-2 ml-3 w-60 rounded-2xl bg-surface shadow-card ring-1 ring-black/5 dark:shadow-none dark:ring-white/5'} {open
      ? 'translate-x-0'
      : '-translate-x-full'}"
    inert={!open}
  >
    <div
      data-tauri-drag-region
      class="flex shrink-0 items-center justify-between px-3 {column ? 'h-[52px]' : 'pt-3 pb-2'}"
    >
      {#if onClose}
        <button
          type="button"
          class="btn-ghost !p-1.5"
          onclick={onClose}
          aria-label={t("chat.closeConversations")}
        >
          <X size={15} aria-hidden="true" />
        </button>
      {/if}
      <span class="pointer-events-none text-[11px] font-semibold text-ink-faint">
        {t("chat.conversations")}
      </span>
      <button
        type="button"
        class="btn-ghost !p-1.5"
        onclick={onNew}
        title={t("chat.newConversation")}
        aria-label={t("chat.newConversation")}
      >
        <Plus size={15} />
      </button>
    </div>
    <div class="shrink-0 px-2 pb-2">
      <!-- Icons centre on this wrapper, which has no padding of its own. -->
      <div class="relative">
        <Search
          size={13}
          class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-faint"
          aria-hidden="true"
        />
        <input
          bind:this={searchInput}
          bind:value={query}
          oninput={onQueryInput}
          onkeydown={onSearchKey}
          type="text"
          role="searchbox"
          aria-label={t("chat.searchConversations")}
          aria-controls={trimmed ? listboxId : undefined}
          aria-activedescendant={active >= 0 ? `${listboxId}-${active}` : undefined}
          placeholder={t("chat.searchConversations")}
          autocomplete="off"
          spellcheck="false"
          class="w-full rounded-lg border border-paper-line bg-paper-soft py-1.5 pr-7 pl-7 text-[12.5px] text-ink outline-none placeholder:text-ink-faint focus:border-navy-500 focus:ring-2 focus:ring-navy-100 dark:bg-white/5 dark:focus:ring-white/10"
        />
        {#if query}
          <button
            type="button"
            class="btn-ghost absolute top-1/2 right-1 -translate-y-1/2 !p-1"
            aria-label={t("chat.clearSearch")}
            title={t("chat.clearSearch")}
            onclick={clearSearch}
          >
            <X size={12} aria-hidden="true" />
          </button>
        {/if}
      </div>
      <div class="sr-only" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
      {#if trimmed}
        {#if rows.length > 0}
          <ChatThreadSearchResults
            {groups}
            activeIndex={active}
            {activeThreadId}
            {listboxId}
            {shelves}
            {onOpen}
          />
        {:else if settled}
          <p class="px-2.5 py-3 text-[12px] break-words text-ink-faint">{announcement}</p>
        {:else}
          <!-- Message matches are still on their way (debounce + index read). -->
          <p class="px-2.5 py-3 text-[12px] text-ink-faint">{t("chat.searching")}</p>
        {/if}
      {:else}
        {#each threads as thread (thread.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="group relative mb-0.5 flex min-h-[52px] w-full items-center rounded-lg
            {activeThreadId === thread.id ? 'bg-paper-soft' : 'hover:bg-paper-soft/60'}"
            oncontextmenu={(event) => onThreadMenu(thread, event)}
          >
            <div class="flex shrink-0 items-center pl-2">
              <ConversationFace avatarId={thread.avatarId} />
            </div>
            {#if editingId === thread.id}
              <label class="sr-only" for="rename-{thread.id}">{t("chat.conversationName")}</label>
              <!-- svelte-ignore a11y_autofocus -->
              <input
                id="rename-{thread.id}"
                class="mx-1.5 min-w-0 flex-1 rounded-md border border-navy-300 bg-surface px-2 py-1 text-[12.5px] text-ink outline-none"
                bind:value={editTitle}
                autofocus
                onblur={() => commitRename(thread.id)}
                onkeydown={(e) => onRenameKey(e, thread.id)}
              />
            {:else}
              <button
                type="button"
                class="flex min-h-[52px] min-w-0 flex-1 items-center gap-2 rounded-lg px-2.5 py-2 text-left"
                onclick={() => onOpen(thread.id)}
                ondblclick={(e) => beginRename(thread, e)}
              >
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-[12.5px] font-medium text-ink">
                    {thread.title}
                  </span>
                  {#if thread.shelfId}
                    {@const subtitle = threadShelfSubtitle(thread, shelves)}
                    {#if subtitle}<span class="block truncate text-[11px] text-ink-faint">
                        {subtitle}
                      </span>{/if}
                  {/if}
                </span>
              </button>
            {/if}
            {#if editingId !== thread.id}
              <div class="flex shrink-0 pr-1">
                <button
                  type="button"
                  class="btn-ghost !p-1"
                  aria-label={t("chat.renameConversation")}
                  title={t("chat.rename")}
                  onclick={(e) => beginRename(thread, e)}
                >
                  <Pencil size={12.5} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  class="btn-ghost !p-1"
                  aria-label={t("chat.deleteConversationAction")}
                  onclick={(e) => {
                    e.stopPropagation();
                    onRemove(thread.id);
                  }}
                >
                  <Trash2 size={12.5} aria-hidden="true" />
                </button>
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
    <div
      class="mx-3 mb-3 flex items-start gap-2.5 rounded-xl border border-navy-200 bg-navy-50 px-3 py-2.5 dark:border-white/10 dark:bg-white/8"
    >
      <Lock size={13} class="mt-0.5 shrink-0 text-navy-700 dark:text-navy-200" />
      <p class="text-[11px] leading-snug text-ink-soft">{t("privacyHere")}</p>
    </div>
  </aside>
</div>
