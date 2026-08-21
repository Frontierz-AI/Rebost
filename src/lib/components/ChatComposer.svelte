<script lang="ts">
  import { api } from "$lib/api";
  import { importIntoChat } from "$lib/chat-import";
  import { fileListQuery, placeholderAt, replacePlaceholder } from "$lib/placeholders";
  import { shelfDisplayName } from "$lib/shelf-label";
  import { app, chatState, fillDraft, logInvokeError, openCreateShelf } from "$lib/stores.svelte";
  import { PROMPT_MAX_CHARS } from "$lib/text-cap";
  import { SendHorizontal, Square, ChevronDown, LibraryBig, Plus, Paperclip } from "@lucide/svelte";

  let {
    composerEl = $bindable(null),
    hasModel,
    generating,
    onSend,
    onStop,
    onChooseShelf,
    onAutoResize,
  }: {
    composerEl: HTMLTextAreaElement | null;
    hasModel: boolean;
    generating: boolean;
    onSend: () => void;
    onStop: () => void;
    onChooseShelf: (id: string | null) => void;
    onAutoResize: () => void;
  } = $props();

  let shelfMenuOpen = $state(false);
  let cursor = $state(0);
  let fileNames = $state<string[]>([]);
  let highlight = $state(0);

  const selectedShelf = $derived.by(() => {
    const id = chatState.selectedShelfId;
    if (!id) return null;
    if (chatState.uploadShelf?.id === id) return null;
    return app.shelves.find((shelf) => shelf.id === id) ?? null;
  });
  const selectedLabel = $derived(selectedShelf ? shelfDisplayName(selectedShelf) : "No Shelf");

  $effect(() => {
    const libraryId = selectedShelf?.id ?? null;
    const uploadId = chatState.uploadShelf?.id ?? null;
    void app.ingestTick;
    if (!libraryId && !uploadId) {
      fileNames = [];
      return;
    }
    let cancelled = false;
    Promise.all([
      libraryId ? api.shelfDocuments(libraryId) : Promise.resolve([]),
      uploadId && uploadId !== libraryId ? api.shelfDocuments(uploadId) : Promise.resolve([]),
    ])
      .then(([libraryDocs, uploadDocs]) => {
        if (!cancelled) {
          fileNames = [...libraryDocs, ...uploadDocs].map((doc) => doc.fileName);
        }
      })
      .catch(() => {
        if (!cancelled) fileNames = [];
      });
    return () => {
      cancelled = true;
    };
  });

  const activeSlot = $derived(placeholderAt(chatState.draft, cursor));
  const fileMatches = $derived.by(() => {
    if (!activeSlot || fileNames.length === 0) return [];
    const query = fileListQuery(activeSlot.inner);
    if (query === null) return [];
    const matches = query
      ? fileNames.filter((name) => name.toLowerCase().includes(query))
      : fileNames;
    return matches.slice(0, 8);
  });
  const listOpen = $derived(fileMatches.length > 0);

  $effect(() => {
    void fileMatches.length;
    highlight = 0;
  });

  function syncCursor() {
    if (composerEl) cursor = composerEl.selectionStart ?? 0;
  }

  function pickFile(name: string) {
    if (!activeSlot) return;
    fillDraft(replacePlaceholder(chatState.draft, activeSlot, name));
  }

  async function attachFiles() {
    await importIntoChat();
  }

  function onKeydown(event: KeyboardEvent) {
    if (listOpen) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        highlight = (highlight + 1) % fileMatches.length;
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        highlight = (highlight - 1 + fileMatches.length) % fileMatches.length;
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        const end = activeSlot?.end ?? cursor;
        cursor = end;
        composerEl?.setSelectionRange(end, end);
        return;
      }
      if (!event.shiftKey && (event.key === "Enter" || event.key === "Tab")) {
        const name = fileMatches[highlight];
        if (name) {
          event.preventDefault();
          pickFile(name);
          return;
        }
      }
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!generating && hasModel) onSend();
    }
  }
</script>

<div class="px-6 pt-3 pb-4">
  <div class="mx-auto max-w-[760px]">
    {#if !hasModel}
      <button
        type="button"
        id="composer-needs-ai"
        class="group mb-2 flex w-full items-center justify-between rounded-lg border border-navy-500/50 bg-navy-100 px-3 py-2 text-[12.5px] text-ink hover:border-navy-500 hover:bg-navy-200/60 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-800"
        onclick={() => (app.view = "settings")}
      >
        <span>To start using Rebost you need to download an AI.</span>
        <span class="font-semibold text-navy-700 group-hover:text-navy-900 dark:text-navy-200 dark:group-hover:text-white"
          >Install</span
        >
      </button>
    {/if}
    <div class="card relative flex flex-col gap-1 !rounded-2xl px-3 pt-2.5 pb-2">
      {#if listOpen}
        <div
          class="absolute right-0 bottom-full left-0 z-30 mb-1 max-h-56 overflow-y-auto rounded-xl border border-paper-line bg-surface py-1 shadow-pop dark:shadow-none"
          role="listbox"
          id="composer-file-list"
          aria-label={selectedShelf && chatState.uploadShelf
            ? "Files on this Shelf and in this conversation"
            : chatState.uploadShelf
              ? "Files in this conversation"
              : "Files on this Shelf"}
        >
          {#each fileMatches as name, index (name)}
            <button
              type="button"
              role="option"
              aria-selected={index === highlight}
              class="flex w-full px-3 py-1.5 text-left text-[12.5px] {index === highlight
                ? 'bg-navy-50 font-medium text-navy-800 dark:bg-white/8 dark:text-ink'
                : 'text-ink hover:bg-paper-soft'}"
              onmousedown={(event) => event.preventDefault()}
              onclick={() => pickFile(name)}
            >
              {name}
            </button>
          {/each}
        </div>
      {/if}
      <textarea
        bind:this={composerEl}
        bind:value={chatState.draft}
        oninput={() => {
          syncCursor();
          onAutoResize();
        }}
        onkeydown={onKeydown}
        onkeyup={syncCursor}
        onclick={syncCursor}
        onselect={syncCursor}
        onfocus={() => {
          syncCursor();
          if (hasModel) api.warmEngine().catch((error) => logInvokeError(error, "warm engine"));
        }}
        rows="1"
        maxlength={PROMPT_MAX_CHARS}
        aria-label="Message Rebost"
        aria-describedby={!hasModel ? "composer-needs-ai" : undefined}
        aria-controls={listOpen ? "composer-file-list" : undefined}
        placeholder={hasModel ? "Message Rebost…" : "Install an AI first…"}
        class="max-h-[180px] w-full cursor-text resize-none bg-transparent text-[13.8px] leading-relaxed outline-none select-text placeholder:text-ink-faint"
      ></textarea>
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-1">
          <div class="relative">
            <button
              type="button"
              class="chip border {selectedShelf
                ? 'border-navy-300 bg-navy-100/70 text-navy-800 dark:border-white/15 dark:bg-white/10 dark:text-navy-100'
                : 'border-paper-line bg-paper-soft text-ink-soft'} hover:border-navy-400"
              onclick={() => (shelfMenuOpen = !shelfMenuOpen)}
              aria-haspopup="listbox"
              aria-expanded={shelfMenuOpen}
              aria-label="Choose a Shelf"
            >
              <LibraryBig size={12.5} aria-hidden="true" />
              {selectedLabel}
              <ChevronDown size={12} />
            </button>
            {#if shelfMenuOpen}
              <div
                class="fixed inset-0 z-20"
                role="presentation"
                onclick={() => (shelfMenuOpen = false)}
                onkeydown={(e) => e.key === "Escape" && (shelfMenuOpen = false)}
              ></div>
              <div
                class="absolute bottom-8 left-0 z-30 w-56 overflow-hidden rounded-xl border border-paper-line bg-surface shadow-pop dark:shadow-none"
                role="listbox"
              >
                <div class="py-1">
                  <button
                    type="button"
                    class="flex w-full items-center gap-2 px-3 py-2 text-left text-[12.5px] hover:bg-paper-soft {chatState.selectedShelfId ===
                    null
                      ? 'font-semibold text-navy-800 dark:text-ink'
                      : 'text-ink'}"
                    onclick={() => {
                      shelfMenuOpen = false;
                      onChooseShelf(null);
                    }}
                  >
                    No Shelf
                    <span class="ml-auto text-[10.5px] text-ink-faint">general questions</span>
                  </button>
                  {#each app.shelves as shelf (shelf.id)}
                    <button
                      type="button"
                      class="flex w-full items-center gap-2 px-3 py-2 text-left text-[12.5px] hover:bg-paper-soft {chatState.selectedShelfId ===
                      shelf.id
                        ? 'font-semibold text-navy-800 dark:text-ink'
                        : 'text-ink'}"
                      onclick={() => {
                        shelfMenuOpen = false;
                        onChooseShelf(shelf.id);
                      }}
                    >
                      <LibraryBig size={12.5} class="text-ink-faint" />
                      {shelfDisplayName(shelf)}
                      <span class="ml-auto text-[10.5px] text-ink-faint"
                        >{shelf.stats.searchable} files</span
                      >
                    </button>
                  {/each}
                </div>
                {#if app.shelves.length === 0}
                  <div class="border-t border-paper-line p-1.5">
                    <button
                      type="button"
                      class="btn-outline w-full !py-1.5 !text-[12.5px]"
                      onclick={() => {
                        shelfMenuOpen = false;
                        openCreateShelf();
                      }}
                    >
                      <Plus size={13} /> New Shelf
                    </button>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
          <button
            type="button"
            class="btn-ghost !rounded-full !p-2"
            onclick={attachFiles}
            title="Add files to this conversation"
            aria-label="Add files to this conversation"
          >
            <Paperclip size={15} aria-hidden="true" />
          </button>
        </div>
        {#if generating}
          <button
            type="button"
            class="btn-primary btn-icon"
            onclick={onStop}
            title="Stop"
            aria-label="Stop generating"
          >
            <Square size={14} fill="currentColor" />
          </button>
        {:else}
          <button
            type="button"
            class="btn-amber btn-icon"
            onclick={onSend}
            disabled={!hasModel || !chatState.draft.trim()}
            title={hasModel ? "Send" : "Install an AI first"}
            aria-label={hasModel ? "Send message" : "Install an AI first"}
            aria-describedby={!hasModel ? "composer-needs-ai" : undefined}
          >
            <SendHorizontal size={15} />
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>
