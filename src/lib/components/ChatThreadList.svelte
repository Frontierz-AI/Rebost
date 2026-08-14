<script lang="ts">
  import type { ShelfView, ThreadMeta } from "$lib/api";
  import { Plus, MessageCircle, Trash2, Lock } from "@lucide/svelte";

  let {
    threads,
    activeThreadId,
    shelves,
    onopen,
    onnew,
    onremove,
  }: {
    threads: ThreadMeta[];
    activeThreadId: string | null;
    shelves: ShelfView[];
    onopen: (id: string) => void;
    onnew: () => void;
    onremove: (id: string, event: MouseEvent) => void;
  } = $props();

  const open = $derived(threads.length > 0);
</script>

<div
  class="flex h-full min-h-0 shrink-0 overflow-hidden transition-[width] duration-300 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none {open
    ? 'w-[15.75rem]'
    : 'w-0'}"
  aria-hidden={!open}
>
  <aside
    class="mt-1 mb-2 ml-3 flex w-60 shrink-0 flex-col overflow-hidden rounded-2xl bg-white shadow-card ring-1 ring-black/5 transition-transform duration-300 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none {open
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
        onclick={onnew}
        title="New conversation"
        aria-label="New conversation"
      >
        <Plus size={15} />
      </button>
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
      {#each threads as thread (thread.id)}
        <div
          class="group relative mb-0.5 flex min-h-[52px] w-full items-center rounded-lg
            {activeThreadId === thread.id ? 'bg-paper-soft' : 'hover:bg-paper-soft/60'}"
        >
          <button
            type="button"
            class="flex min-h-[52px] w-full items-center gap-2 rounded-lg px-2.5 py-2 pr-8 text-left"
            onclick={() => onopen(thread.id)}
          >
            <MessageCircle size={14} class="shrink-0 text-ink-faint" />
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[12.5px] font-medium text-ink">{thread.title}</span>
              {#if thread.shelfId}
                {@const shelf = shelves.find((s) => s.id === thread.shelfId)}
                {#if shelf}<span class="block truncate text-[11px] text-ink-faint"
                    >{shelf.name}</span
                  >{/if}
              {/if}
            </span>
          </button>
          <button
            type="button"
            class="btn-ghost absolute top-1/2 right-1.5 -translate-y-1/2 !p-1"
            aria-label="Delete conversation"
            onclick={(e) => onremove(thread.id, e)}
          >
            <Trash2 size={12.5} />
          </button>
        </div>
      {/each}
    </div>
    <div
      class="mx-3 mb-3 flex items-start gap-2.5 rounded-xl border border-amber-450/35 bg-amber-350/10 px-3 py-2.5"
    >
      <Lock size={13} class="mt-0.5 shrink-0 text-amber-550" />
      <p class="text-[11px] leading-snug text-ink-soft">
        Private AI that lives with your files and never leaves them.
      </p>
    </div>
  </aside>
</div>
