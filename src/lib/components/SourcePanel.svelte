<script lang="ts">
  import type { SourcePassage } from "$lib/api";
  import { api } from "$lib/api";
  import { notifyInvokeError } from "$lib/stores.svelte";
  import CopyActions from "./CopyActions.svelte";
  import Markdown from "./Markdown.svelte";
  import { overlay, sheetPanel } from "$lib/motion";
  import { focusTrap } from "$lib/focus-trap";
  import { X, ExternalLink, FileText, FolderOpen } from "@lucide/svelte";

  let { source, onClose }: { source: SourcePassage; onClose: () => void } = $props();

  let body = $state("");
  let missing = $state(false);

  $effect(() => {
    const passage = source;
    missing = false;
    if (passage.body) {
      body = passage.body;
      return;
    }
    body = "";
    if (!passage.shelfId || !passage.documentId) {
      missing = true;
      return;
    }
    let cancelled = false;
    api
      .documentText(passage.shelfId, passage.documentId)
      .then((text) => {
        if (!cancelled) body = text;
      })
      .catch(() => {
        if (!cancelled) missing = true;
      });
    return () => {
      cancelled = true;
    };
  });

  const location = $derived.by(() => {
    const parts: string[] = [];
    if (source.pageStart) {
      parts.push(
        source.pageEnd && source.pageEnd !== source.pageStart
          ? `Page ${source.pageStart}–${source.pageEnd}`
          : `Page ${source.pageStart}`,
      );
    }
    if (source.section) parts.push(source.section);
    return parts.join(" · ");
  });
</script>

<div
  class="fixed inset-0 z-40 flex items-end justify-end bg-navy-950/20 p-5 dark:bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label={source.title}
  tabindex="-1"
  use:focusTrap
  transition:overlay
  onclick={(e) => e.target === e.currentTarget && onClose()}
  onkeydown={(e) => e.key === "Escape" && onClose()}
>
  <div
    class="card z-50 flex max-h-[70vh] w-[430px] flex-col overflow-hidden shadow-pop dark:shadow-none"
    in:sheetPanel
  >
    <div class="flex items-start gap-3 border-b border-paper-line bg-paper-soft px-4 py-3">
      <span
        class="mt-0.5 rounded-md bg-navy-100 p-1.5 text-navy-700 dark:bg-white/10 dark:text-navy-200"
        ><FileText size={15} /></span
      >
      <div class="min-w-0 flex-1">
        <p class="truncate text-[13px] font-semibold text-ink">{source.title}</p>
        {#if location}<p class="text-[12px] text-ink-soft">{location}</p>{/if}
      </div>
      <span class="rounded-md bg-navy-900 px-1.5 py-0.5 text-[10.5px] font-bold text-white"
        >{source.sid}</span
      >
      <button type="button" class="btn-ghost !p-1" onclick={onClose} aria-label="Close"
        ><X size={15} /></button
      >
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto px-4 py-3">
      {#if body}
        <Markdown text={body} />
      {:else if missing}
        <p class="text-[13px] text-ink-soft">This file isn't available.</p>
      {:else}
        <p class="flex items-center gap-2 text-[13px] text-ink-soft">
          <span class="inline-block h-2 w-2 animate-pulse rounded-full bg-navy-500"></span>
          <span class="sr-only">Loading</span>
        </p>
      {/if}
    </div>
    <div class="flex items-center justify-between gap-2 border-t border-paper-line px-3 py-2">
      <CopyActions text={body} />
      <div class="flex gap-1.5">
        <button
          type="button"
          class="btn-outline !py-1.5 !text-[12px]"
          onclick={() => api.revealItem(source.path).catch(notifyInvokeError)}
        >
          <FolderOpen size={13} /> Show in folder
        </button>
        <button
          type="button"
          class="btn-outline !py-1.5 !text-[12px]"
          onclick={() => api.openOriginal(source.path).catch(notifyInvokeError)}
        >
          <ExternalLink size={13} /> Open original
        </button>
      </div>
    </div>
  </div>
</div>
