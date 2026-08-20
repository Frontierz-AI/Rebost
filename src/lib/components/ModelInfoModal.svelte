<script lang="ts">
  import {
    api,
    catalogHostLabel,
    formatBytes,
    formatCount,
    type ModelSearchResult,
  } from "$lib/api";
  import { dialogPanel, overlay } from "$lib/motion";
  import { focusTrap } from "$lib/focus-trap";
  import { notifyInvokeError } from "$lib/stores.svelte";
  import { Download, ExternalLink, X } from "@lucide/svelte";

  let {
    result,
    installing = false,
    onClose,
    onInstall,
  }: {
    result: ModelSearchResult;
    installing?: boolean;
    onClose: () => void;
    onInstall: () => void;
  } = $props();

  function catalogLabel(source: string): string {
    if (source === "huggingface") return "Hugging Face";
    if (source === "ollama") return "Ollama";
    if (source === "huggingface+ollama") return "Hugging Face + Ollama";
    return source.replace("+", " + ");
  }

  const rows = $derived(
    (
      [
        ["Publisher", result.publisher],
        ["Catalog", catalogLabel(result.source)],
        ["Reference", result.reference],
        ["File", result.file],
        ["Size", result.sizeBytes != null ? formatBytes(result.sizeBytes) : null],
        [
          "Downloads",
          result.downloads != null && result.downloads > 0 ? formatCount(result.downloads) : null,
        ],
        ["Released", result.released],
        ["License", result.license],
        [
          "This computer",
          result.fits === true
            ? "Fits this computer"
            : result.fits === false
              ? "Too large for this computer"
              : null,
        ],
      ] satisfies [string, string | null | undefined][]
    ).filter((row): row is [string, string] => typeof row[1] === "string" && row[1].length > 0),
  );
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-navy-950/25 p-6 dark:bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-labelledby="model-info-title"
  id="model-info-dialog"
  tabindex="-1"
  use:focusTrap
  transition:overlay
  onclick={(e) => e.target === e.currentTarget && onClose()}
  onkeydown={(e) => {
    if (e.key !== "Escape") return;
    e.stopPropagation();
    onClose();
  }}
>
  <div class="card w-full max-w-[440px] shadow-pop dark:shadow-none" in:dialogPanel>
    <div class="flex items-start justify-between gap-3 px-5 pt-5 pb-3">
      <div class="min-w-0">
        <h2 id="model-info-title" class="text-[16px] font-semibold break-words text-ink">
          {result.name}
        </h2>
        {#if result.official}
          <span
            class="mt-2 inline-flex rounded-full bg-navy-100 px-2 py-0.5 text-[10px] font-semibold text-navy-800 dark:bg-white/10 dark:text-navy-100"
          >
            Official
          </span>
        {/if}
      </div>
      <button type="button" class="btn-ghost !p-1.5" aria-label="Close" onclick={onClose}>
        <X size={15} aria-hidden="true" />
      </button>
    </div>
    <dl class="grid grid-cols-2 gap-x-4 gap-y-2.5 px-5 pb-4 text-[12.5px]">
      {#each rows as [label, value] (label)}
        <div class={label === "Reference" || label === "File" ? "col-span-2" : ""}>
          <dt class="label">{label}</dt>
          <dd class="mt-0.5 break-all text-ink">{value}</dd>
        </div>
      {/each}
    </dl>
    <div class="flex items-center justify-end gap-2 border-t border-paper-line px-5 py-3">
      <button
        type="button"
        class="btn-ghost mr-auto !px-2 !py-1 !text-[12.5px]"
        onclick={() => api.openModelPage(result.source, result.reference).catch(notifyInvokeError)}
      >
        <ExternalLink size={12} aria-hidden="true" />
        More on {catalogHostLabel(result.source)}
      </button>
      <button type="button" class="btn-outline" onclick={onClose}>Close</button>
      <button type="button" class="btn-primary" onclick={onInstall} disabled={installing}>
        <Download size={13.5} aria-hidden="true" /> Install
      </button>
    </div>
  </div>
</div>
