<script lang="ts">
  import { fileTypeLabel, formatWhen, type DocumentMeta } from "$lib/api";

  let {
    visibleDocs,
    onopen,
    onrowkey,
    onclear,
  }: {
    visibleDocs: DocumentMeta[];
    onopen: (doc: DocumentMeta) => void;
    onrowkey: (event: KeyboardEvent, doc: DocumentMeta) => void;
    onclear: () => void;
  } = $props();

  const columns = [
    { header: "File", colClass: "", thClass: "pr-2.5 pl-8" },
    { header: "Source", colClass: "w-44", thClass: "px-2.5" },
    { header: "Type", colClass: "w-28", thClass: "px-2.5" },
    { header: "Status", colClass: "w-28", thClass: "px-2.5" },
    { header: "PII", colClass: "w-14", thClass: "px-2.5 text-right" },
    { header: "Updated", colClass: "w-36", thClass: "pr-8 pl-2.5" },
  ] as const;
</script>

{#if visibleDocs.length === 0}
  <div class="flex flex-col items-center justify-center gap-4 px-8 py-16 text-center">
    <div class="flex flex-col items-center gap-1">
      <p class="text-sm font-medium text-ink">No files match these filters</p>
      <p class="max-w-[40ch] text-pretty text-[13px] text-ink-soft">
        Try a different name, source, or type.
      </p>
    </div>
    <button type="button" class="btn-ghost" onclick={onclear}>Clear filters</button>
  </div>
{:else}
  <table class="w-full table-fixed border-separate border-spacing-0 text-[12.8px]">
    <colgroup>
      {#each columns as column}
        <col class={column.colClass} />
      {/each}
    </colgroup>
    <thead>
      <tr class="text-left">
        {#each columns as column}
          <th
            scope="col"
            class="sticky top-0 z-10 border-b border-paper-line bg-paper py-2 font-medium whitespace-nowrap text-ink-faint {column.thClass}"
            >{column.header}</th
          >
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each visibleDocs as doc (doc.id)}
        <tr
          class="cursor-pointer hover:bg-navy-50/60 focus-visible:bg-navy-50 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-navy-500"
          tabindex="0"
          role="button"
          aria-label={doc.fileName}
          onclick={() => onopen(doc)}
          onkeydown={(e) => onrowkey(e, doc)}
        >
          <td
            class="truncate border-b border-paper-line/70 py-2.5 pr-2.5 pl-8 font-medium text-ink"
            title={doc.relPath}
          >
            {doc.fileName}
          </td>
          <td
            class="truncate border-b border-paper-line/70 px-2.5 py-2.5 text-ink-soft"
            title={doc.sourceType === "imported" ? "Imported" : doc.sourceLabel}
          >
            {doc.sourceType === "imported" ? "Imported" : `Linked · ${doc.sourceLabel}`}
          </td>
          <td class="truncate border-b border-paper-line/70 px-2.5 py-2.5 text-ink-soft"
            >{fileTypeLabel(doc)}</td
          >
          <td class="border-b border-paper-line/70 px-2.5 py-2.5">
            {#if doc.status === "ready"}
              <span class="text-emerald-700">Ready</span>
            {:else if doc.status === "reading"}
              <span class="inline-flex items-center gap-1.5 text-amber-550">
                <span
                  class="inline-block size-1.5 animate-pulse rounded-full bg-amber-450"
                  aria-hidden="true"
                ></span>
                Reading
              </span>
            {:else}
              <span class="text-red-700/80" title={doc.error}>Error</span>
            {/if}
          </td>
          <td
            class="border-b border-paper-line/70 px-2.5 py-2.5 text-right tabular-nums text-ink-soft"
          >
            {doc.piiTotal > 0 ? doc.piiTotal : "—"}
          </td>
          <td class="truncate border-b border-paper-line/70 py-2.5 pr-8 pl-2.5 text-ink-soft"
            >{formatWhen(doc.updatedAt)}</td
          >
        </tr>
      {/each}
    </tbody>
  </table>
{/if}
