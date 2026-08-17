<script lang="ts">
  import { formatWhen, type ShelfView, type ThreadMeta } from "$lib/api";
  import { threadShelfSubtitle } from "$lib/shelf-label";
  import { Download } from "@lucide/svelte";

  let {
    thread,
    shelves,
    messageCount,
    onDownload,
  }: {
    thread: ThreadMeta;
    shelves: ShelfView[];
    messageCount: number;
    onDownload: () => void;
  } = $props();

  const started = $derived(startedLabel(thread.createdAt));
  const shelf = $derived(threadShelfSubtitle(thread, shelves));
  const canDownload = $derived(thread.messageCount > 0);

  function startedLabel(iso: string): string {
    const when = formatWhen(iso);
    switch (when) {
      case "Today":
        return "Started today";
      case "Yesterday":
        return "Started yesterday";
      default:
        return `Started ${when}`;
    }
  }
</script>

<header class="flex items-center gap-3 py-1" aria-label="Conversation details">
  <div
    class="h-px min-w-0 flex-1 bg-linear-to-r from-transparent to-paper-line"
    aria-hidden="true"
  ></div>
  <div class="flex min-w-0 items-center gap-2.5 bg-paper px-2.5">
    <ul role="list" class="flex min-w-0 items-center gap-2 text-[0.75rem] leading-4 text-ink-faint">
      <li class="truncate">{started}</li>
      {#if messageCount > 0}
        <li class="flex items-center gap-2 whitespace-nowrap tabular-nums">
          <span aria-hidden="true">·</span>
          {messageCount === 1 ? "1 message" : `${messageCount} messages`}
        </li>
      {/if}
      {#if shelf}
        <li class="flex max-w-[10rem] min-w-0 items-center gap-2">
          <span aria-hidden="true">·</span>
          <div class="min-w-0 truncate">{shelf}</div>
        </li>
      {/if}
    </ul>
    {#if canDownload}
      <span class="h-3 w-px shrink-0 bg-paper-line" aria-hidden="true"></span>
      <button
        type="button"
        class="relative inline-flex shrink-0 items-center gap-1.5 rounded-lg py-1 pr-2.5 pl-1.5 text-[0.75rem] text-ink-faint hover:bg-navy-900/6 hover:text-ink"
        onclick={onDownload}
        aria-label="Download this conversation"
        title="Download"
      >
        <Download size={14} class="shrink-0" aria-hidden="true" />
        Download
        <span
          class="absolute top-1/2 left-1/2 size-[max(100%,3rem)] -translate-1/2 pointer-fine:hidden"
          aria-hidden="true"
        ></span>
      </button>
    {/if}
  </div>
  <div
    class="h-px min-w-0 flex-1 bg-linear-to-l from-transparent to-paper-line"
    aria-hidden="true"
  ></div>
</header>
