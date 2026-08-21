<script lang="ts">
  import {
    api,
    formatBytes,
    formatWhen,
    PII_EMPTY_HINT,
    piiLabel,
    type Card,
    type DocumentMeta,
  } from "$lib/api";
  import CopyActions from "$lib/components/CopyActions.svelte";
  import { notifyInvokeError } from "$lib/stores.svelte";
  import { drawerPanel, overlay } from "$lib/motion";
  import { focusTrap } from "$lib/focus-trap";
  import { FolderOpen, FileText, RefreshCw, ScanText, X } from "@lucide/svelte";

  let {
    doc,
    card,
    extractedText = $bindable(null),
    onClose,
  }: {
    doc: DocumentMeta;
    card: Card | null;
    extractedText: string | null;
    onClose: () => void;
  } = $props();

  function statusLabel(document: DocumentMeta): string {
    if (document.status === "ready") return "Ready to use";
    if (document.status === "reading") return "Reading…";
    return document.error ?? "Couldn't read";
  }

  async function viewExtracted() {
    try {
      extractedText = await api.documentText(doc.shelfId, doc.id);
    } catch (error) {
      notifyInvokeError(error);
    }
  }
</script>

<div
  class="fixed inset-0 z-40 flex items-stretch justify-end bg-navy-950/25 dark:bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label={doc.fileName}
  tabindex="-1"
  use:focusTrap
  transition:overlay
  onclick={(e) => e.target === e.currentTarget && onClose()}
  onkeydown={(e) => e.key === "Escape" && onClose()}
>
  <div
    class="flex h-full w-[480px] flex-col overflow-hidden bg-surface shadow-pop dark:shadow-none"
    in:drawerPanel
  >
    <div class="border-b border-paper-line bg-paper-soft px-5 py-4">
      <div class="flex items-start justify-between gap-3">
        <div class="flex min-w-0 items-start gap-3">
          <span
            class="mt-0.5 rounded-lg bg-navy-100 p-2 text-navy-700 dark:bg-white/10 dark:text-navy-200"
            ><FileText size={16} /></span
          >
          <div class="min-w-0">
            <p class="truncate text-[14.5px] font-semibold text-ink" title={doc.fileName}>
              {doc.fileName}
            </p>
            <p
              class="mt-0.5 text-[12px] {doc.status === 'error'
                ? 'text-red-700/80 dark:text-red-400'
                : doc.status === 'ready'
                  ? 'text-ready-ink dark:text-navy-300'
                  : 'text-amber-550'}"
            >
              {statusLabel(doc)}
            </p>
          </div>
        </div>
        <button type="button" class="btn-ghost !p-1.5" aria-label="Close" onclick={onClose}
          ><X size={15} /></button
        >
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      {#if extractedText !== null}
        <div class="mb-3 flex items-center justify-between">
          <span class="label">Text as Rebost reads it</span>
          <button
            type="button"
            class="btn-ghost !py-1 !text-[11.5px]"
            onclick={() => (extractedText = null)}>Back to details</button
          >
        </div>
        <pre
          class="rounded-lg border border-paper-line bg-paper-soft p-3 text-[11.5px] leading-relaxed whitespace-pre-wrap">{extractedText}</pre>
      {:else}
        <dl class="grid grid-cols-2 gap-x-4 gap-y-2.5 text-[12.5px]">
          <div>
            <dt class="label">Source</dt>
            <dd class="mt-0.5 text-ink">
              {doc.sourceType === "imported" ? "Imported" : `Linked · ${doc.sourceLabel}`}
            </dd>
          </div>
          <div>
            <dt class="label">Size</dt>
            <dd class="mt-0.5 text-ink">{formatBytes(doc.sizeBytes)}</dd>
          </div>
          {#if doc.pages}<div>
              <dt class="label">Pages</dt>
              <dd class="mt-0.5 text-ink">{doc.pages}</dd>
            </div>{/if}
          <div>
            <dt class="label">Searchable passages</dt>
            <dd class="mt-0.5 text-ink">{doc.passageCount}</dd>
          </div>
          {#if card?.language}<div>
              <dt class="label">Language</dt>
              <dd class="mt-0.5 text-ink uppercase">{card.language}</dd>
            </div>{/if}
          <div>
            <dt class="label">Updated</dt>
            <dd class="mt-0.5 text-ink">{formatWhen(doc.updatedAt)}</dd>
          </div>
        </dl>

        {#if doc.ocr}
          <p
            class="mt-3 flex items-center gap-2 rounded-lg bg-navy-50 px-3 py-2 text-[12px] text-navy-800 dark:bg-white/8 dark:text-navy-100"
          >
            <ScanText size={13.5} /> This file had no selectable text, so Rebost read it as a picture
            on this computer.
          </p>
        {/if}

        {#if card}
          {#if card.summary}
            <h3 class="label mt-5 mb-1.5">Summary</h3>
            <p class="text-[13px] leading-relaxed text-ink">{card.summary}</p>
          {/if}
          {#if card.keywords.length > 0}
            <h3 class="label mt-4 mb-1.5">Keywords</h3>
            <div class="flex flex-wrap gap-1.5">
              {#each card.keywords as keyword}
                <span class="chip !cursor-default bg-paper-soft text-ink-soft">{keyword}</span>
              {/each}
            </div>
          {/if}
          {#if card.outline.length > 0}
            <h3 class="label mt-4 mb-1.5">Outline</h3>
            <ul class="space-y-1">
              {#each card.outline.slice(0, 14) as entry}
                <li class="flex items-baseline justify-between gap-3 text-[12.5px]">
                  <span class="truncate text-ink">{entry.title}</span>
                  {#if entry.page}<span class="shrink-0 text-ink-faint">p. {entry.page}</span>{/if}
                </li>
              {/each}
            </ul>
          {/if}
        {/if}

        {#if doc.piiTotal > 0}
          <h3 class="label mt-5 mb-1.5">Personal information</h3>
          <div class="rounded-lg border border-paper-line bg-paper-soft/60 px-3.5 py-2.5">
            {#each Object.entries(doc.piiCategories ?? {}) as [category, count]}
              <p class="flex justify-between py-0.5 text-[12.5px]">
                <span class="text-ink-soft">{piiLabel(category, count)}</span>
                <span class="font-semibold text-ink tabular-nums">{count}</span>
              </p>
            {/each}
          </div>
        {:else if doc.status === "ready"}
          <h3 class="label mt-5 mb-1.5">Personal information</h3>
          <p class="text-[12px] text-ink-faint">{PII_EMPTY_HINT}</p>
        {/if}
      {/if}
    </div>

    <div class="flex flex-col gap-2 border-t border-paper-line px-4 py-3">
      {#if extractedText !== null}
        <CopyActions text={extractedText} />
      {/if}
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="btn-outline shrink-0 !text-[12px] whitespace-nowrap"
          onclick={() => api.openOriginal(doc.path).catch(notifyInvokeError)}
        >
          <FileText size={13} class="shrink-0" aria-hidden="true" /> Open original
        </button>
        <button
          type="button"
          class="btn-outline shrink-0 !text-[12px] whitespace-nowrap"
          onclick={() => api.revealItem(doc.path).catch(notifyInvokeError)}
        >
          <FolderOpen size={13} class="shrink-0" aria-hidden="true" /> Show in folder
        </button>
        {#if doc.status === "ready" && extractedText === null}
          <button
            type="button"
            class="btn-outline shrink-0 !text-[12px] whitespace-nowrap"
            onclick={viewExtracted}
          >
            <ScanText size={13} class="shrink-0" aria-hidden="true" /> View the text
          </button>
        {/if}
        {#if doc.status === "error"}
          <button
            type="button"
            class="btn-amber shrink-0 !text-[12px] whitespace-nowrap"
            onclick={() =>
              api
                .documentReprocess(doc.shelfId, doc.id)
                .then(() => onClose())
                .catch(notifyInvokeError)}
          >
            <RefreshCw size={13} class="shrink-0" aria-hidden="true" /> Try again
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>
