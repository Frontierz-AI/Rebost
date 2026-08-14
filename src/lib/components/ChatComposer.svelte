<script lang="ts">
  import { api, type ShelfView } from "$lib/api";
  import { app, chatState, openCreateShelf } from "$lib/stores.svelte";
  import { SendHorizontal, Square, ChevronDown, LibraryBig, Plus } from "@lucide/svelte";

  let {
    composerEl = $bindable(null),
    hasModel,
    generating,
    selectedShelfId = $bindable(null),
    selectedShelf,
    shelfMenuOpen = $bindable(false),
    onsend,
    onstop,
    onchooseshelf,
    onautoresize,
  }: {
    composerEl: HTMLTextAreaElement | null;
    hasModel: boolean;
    generating: boolean;
    selectedShelfId: string | null;
    selectedShelf: ShelfView | null;
    shelfMenuOpen: boolean;
    onsend: () => void;
    onstop: () => void;
    onchooseshelf: (id: string | null) => void;
    onautoresize: () => void;
  } = $props();

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onsend();
    }
  }
</script>

<div class="px-6 pt-3 pb-4">
  <div class="mx-auto max-w-[760px]">
    {#if !hasModel}
      <button
        type="button"
        class="mb-2 flex w-full items-center justify-between rounded-lg border border-amber-450/50 bg-amber-350/20 px-3 py-2 text-[12.5px] text-ink"
        onclick={() => (app.view = "settings")}
      >
        <span>Rebost needs a model before it can answer. Installing it takes a click.</span>
        <span class="font-semibold text-navy-700">Open Settings →</span>
      </button>
    {/if}
    <div class="card flex flex-col gap-1 !rounded-2xl px-3 pt-2.5 pb-2">
      <textarea
        bind:this={composerEl}
        bind:value={chatState.draft}
        oninput={onautoresize}
        onkeydown={onKeydown}
        onfocus={() => hasModel && api.warmEngine()}
        rows="1"
        aria-label="Message Rebost"
        placeholder="Message Rebost…"
        class="max-h-[180px] w-full resize-none bg-transparent text-[13.8px] leading-relaxed outline-none placeholder:text-ink-faint select-text cursor-text"
      ></textarea>
      <div class="flex items-center justify-between">
        <div class="relative">
          <button
            type="button"
            class="chip border {selectedShelf
              ? 'border-navy-300 bg-navy-100/70 text-navy-800'
              : 'border-paper-line bg-paper-soft text-ink-soft'} hover:border-navy-400"
            onclick={() => (shelfMenuOpen = !shelfMenuOpen)}
            aria-haspopup="listbox"
            aria-expanded={shelfMenuOpen}
            aria-label="Choose a Shelf"
          >
            <LibraryBig size={12.5} />
            {selectedShelf ? selectedShelf.name : "No Shelf"}
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
              class="absolute bottom-8 left-0 z-30 w-56 overflow-hidden rounded-xl border border-paper-line bg-white shadow-pop"
              role="listbox"
            >
              <div class="py-1">
                <button
                  type="button"
                  class="flex w-full items-center gap-2 px-3 py-2 text-left text-[12.5px] hover:bg-paper-soft {selectedShelfId ===
                  null
                    ? 'font-semibold text-navy-800'
                    : 'text-ink'}"
                  onclick={() => onchooseshelf(null)}
                >
                  No Shelf
                  <span class="ml-auto text-[10.5px] text-ink-faint">general questions</span>
                </button>
                {#each app.shelves as shelf (shelf.id)}
                  <button
                    type="button"
                    class="flex w-full items-center gap-2 px-3 py-2 text-left text-[12.5px] hover:bg-paper-soft {selectedShelfId ===
                    shelf.id
                      ? 'font-semibold text-navy-800'
                      : 'text-ink'}"
                    onclick={() => onchooseshelf(shelf.id)}
                  >
                    <LibraryBig size={12.5} class="text-ink-faint" />
                    {shelf.name}
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
        {#if generating}
          <button
            type="button"
            class="btn-primary !rounded-full !p-2.5"
            onclick={onstop}
            title="Stop"
            aria-label="Stop generating"
          >
            <Square size={14} fill="currentColor" />
          </button>
        {:else}
          <button
            type="button"
            class="btn-amber !rounded-full !p-2.5"
            onclick={onsend}
            disabled={!chatState.draft.trim()}
            title="Send"
            aria-label="Send message"
          >
            <SendHorizontal size={15} />
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>
