<script lang="ts">
  import type { SourcePassage } from "$lib/api";
  import { api } from "$lib/api";
  import CopyActions from "./CopyActions.svelte";
  import { focusTrap } from "$lib/focus-trap";
  import { X, ExternalLink, FileText, FolderOpen } from "@lucide/svelte";

  let { source, onclose }: { source: SourcePassage; onclose: () => void } = $props();

  const location = $derived.by(() => {
    const parts: string[] = [];
    if (source.pageStart) {
      parts.push(
        source.pageEnd && source.pageEnd !== source.pageStart
          ? `p. ${source.pageStart}–${source.pageEnd}`
          : `p. ${source.pageStart}`,
      );
    }
    if (source.section) parts.push(source.section);
    return parts.join(" · ");
  });
</script>

<div
  class="fixed inset-0 z-40 flex items-end justify-end bg-navy-950/20 p-5"
  role="dialog"
  aria-modal="true"
  aria-label={source.title}
  tabindex="-1"
  use:focusTrap
  onclick={(e) => e.target === e.currentTarget && onclose()}
  onkeydown={(e) => e.key === "Escape" && onclose()}
>
  <div class="card z-50 flex max-h-[70vh] w-[430px] flex-col overflow-hidden shadow-pop">
    <div class="flex items-start gap-3 border-b border-paper-line bg-paper-soft px-4 py-3">
      <span class="mt-0.5 rounded-md bg-navy-100 p-1.5 text-navy-700"><FileText size={15} /></span>
      <div class="min-w-0 flex-1">
        <p class="truncate text-[13px] font-semibold text-ink">{source.title}</p>
        {#if location}<p class="text-[12px] text-ink-soft">{location}</p>{/if}
      </div>
      <span class="rounded-md bg-navy-900 px-1.5 py-0.5 text-[10.5px] font-bold text-white"
        >{source.sid}</span
      >
      <button type="button" class="btn-ghost !p-1" onclick={onclose} aria-label="Close"
        ><X size={15} /></button
      >
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto px-4 py-3">
      <p class="text-[13px] leading-relaxed whitespace-pre-wrap text-ink">{source.body}</p>
    </div>
    <div class="flex items-center justify-between gap-2 border-t border-paper-line px-3 py-2">
      <CopyActions text={source.body} />
      <div class="flex gap-1.5">
        <button
          type="button"
          class="btn-outline !py-1.5 !text-[12px]"
          onclick={() => api.revealItem(source.path)}
        >
          <FolderOpen size={13} /> Show in folder
        </button>
        <button
          type="button"
          class="btn-outline !py-1.5 !text-[12px]"
          onclick={() => api.openOriginal(source.path)}
        >
          <ExternalLink size={13} /> Open original
        </button>
      </div>
    </div>
  </div>
</div>
