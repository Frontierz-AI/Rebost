<script lang="ts">
  import type { ShelfView, ThreadMeta } from "$lib/api";
  import { threadShelfSubtitle } from "$lib/shelf-label";
  import { popupNativeMenu, type NativeMenuEntry } from "$lib/native-menu";
  import { Plus, MessageCircle, Trash2, Lock, Pencil } from "@lucide/svelte";

  let {
    threads,
    activeThreadId,
    shelves,
    onOpen,
    onNew,
    onRemove,
    onRename,
    onExport,
  }: {
    threads: ThreadMeta[];
    activeThreadId: string | null;
    shelves: ShelfView[];
    onOpen: (id: string) => void;
    onNew: () => void;
    onRemove: (id: string) => void;
    onRename: (id: string, title: string) => void;
    onExport: (id: string) => void;
  } = $props();

  const open = $derived(threads.length > 0);
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

  function onThreadMenu(thread: ThreadMeta, event: MouseEvent) {
    if (editingId === thread.id) return;
    event.preventDefault();
    event.stopPropagation();
    const entries: NativeMenuEntry[] = [
      { kind: "item", text: "Rename", action: () => beginRename(thread) },
    ];
    if (thread.messageCount > 0) {
      entries.push({
        kind: "item",
        text: "Download",
        action: () => onExport(thread.id),
      });
    }
    entries.push(
      { kind: "separator" },
      { kind: "item", text: "Delete", action: () => onRemove(thread.id) },
    );
    void popupNativeMenu(entries).catch(() => {});
  }
</script>

<div
  class="flex h-full min-h-0 shrink-0 overflow-hidden transition-[width] duration-300 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none {open
    ? 'w-[15.75rem]'
    : 'w-0'}"
  aria-hidden={!open}
>
  <aside
    class="mt-1 mb-2 ml-3 flex w-60 shrink-0 flex-col overflow-hidden rounded-2xl bg-surface shadow-card ring-1 ring-black/5 transition-transform duration-300 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none dark:shadow-none dark:ring-white/5 {open
      ? 'translate-x-0'
      : '-translate-x-full'}"
    inert={!open}
  >
    <div data-tauri-drag-region class="flex items-center justify-between px-3 pt-3 pb-2">
      <span class="pointer-events-none text-[11px] font-semibold text-ink-faint">Conversations</span
      >
      <button
        type="button"
        class="btn-ghost !p-1.5"
        onclick={onNew}
        title="New conversation"
        aria-label="New conversation"
      >
        <Plus size={15} />
      </button>
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
      {#each threads as thread (thread.id)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="group relative mb-0.5 flex min-h-[52px] w-full items-center rounded-lg
            {activeThreadId === thread.id ? 'bg-paper-soft' : 'hover:bg-paper-soft/60'}"
          oncontextmenu={(event) => onThreadMenu(thread, event)}
        >
          {#if editingId === thread.id}
            <label class="sr-only" for="rename-{thread.id}">Conversation name</label>
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
              <MessageCircle size={14} class="shrink-0 text-ink-faint" />
              <span class="min-w-0 flex-1">
                <span class="block truncate text-[12.5px] font-medium text-ink">{thread.title}</span
                >
                {#if thread.shelfId}
                  {@const subtitle = threadShelfSubtitle(thread, shelves)}
                  {#if subtitle}<span class="block truncate text-[11px] text-ink-faint"
                      >{subtitle}</span
                    >{/if}
                {/if}
              </span>
            </button>
          {/if}
          {#if editingId !== thread.id}
            <div class="flex shrink-0 pr-1">
              <button
                type="button"
                class="btn-ghost !p-1"
                aria-label="Rename conversation"
                title="Rename"
                onclick={(e) => beginRename(thread, e)}
              >
                <Pencil size={12.5} aria-hidden="true" />
              </button>
              <button
                type="button"
                class="btn-ghost !p-1"
                aria-label="Delete conversation"
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
    </div>
    <div
      class="mx-3 mb-3 flex items-start gap-2.5 rounded-xl border border-amber-450/35 bg-amber-350/10 px-3 py-2.5"
    >
      <Lock size={13} class="mt-0.5 shrink-0 text-amber-550" />
      <p class="text-[11px] leading-snug text-ink-soft">What happens here stays here.</p>
    </div>
  </aside>
</div>
