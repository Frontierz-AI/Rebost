<script lang="ts">
  import { formatBytes, formatCount, type ModelSearchResult } from "$lib/api";
  import { focusTrap } from "$lib/focus-trap";
  import { Download, X } from "@lucide/svelte";

  let {
    result,
    installing = false,
    onclose,
    oninstall,
  }: {
    result: ModelSearchResult;
    installing?: boolean;
    onclose: () => void;
    oninstall: () => void;
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
  class="fixed inset-0 z-40 flex items-center justify-center bg-navy-950/25 p-6"
  role="dialog"
  aria-modal="true"
  aria-labelledby="model-info-title"
  id="model-info-dialog"
  tabindex="-1"
  use:focusTrap
  onclick={(e) => e.target === e.currentTarget && onclose()}
  onkeydown={(e) => e.key === "Escape" && onclose()}
>
  <div class="card w-full max-w-[440px] shadow-pop">
    <div class="flex items-start justify-between gap-3 px-5 pt-5 pb-3">
      <div class="min-w-0">
        <h2 id="model-info-title" class="text-[16px] font-semibold break-words text-ink">
          {result.name}
        </h2>
        {#if result.official}
          <span
            class="mt-2 inline-flex rounded-full bg-navy-100 px-2 py-0.5 text-[10px] font-semibold text-navy-800"
          >
            Official
          </span>
        {/if}
      </div>
      <button type="button" class="btn-ghost !p-1.5" aria-label="Close" onclick={onclose}>
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
      <button type="button" class="btn-outline" onclick={onclose}>Close</button>
      <button type="button" class="btn-primary" onclick={oninstall} disabled={installing}>
        <Download size={13.5} aria-hidden="true" /> Install
      </button>
    </div>
  </div>
</div>
