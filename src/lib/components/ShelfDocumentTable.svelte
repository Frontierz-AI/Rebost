<script lang="ts">
  import { fileTypeLabel, formatWhen, type DocumentMeta } from "$lib/api";

  let {
    visibleDocs,
    onOpen,
    onRetry,
    onClear,
  }: {
    visibleDocs: DocumentMeta[];
    onOpen: (doc: DocumentMeta) => void;
    onRetry: (doc: DocumentMeta) => void;
    onClear: () => void;
  } = $props();

  const columns = [
    { header: "File", colClass: "", thClass: "pr-2.5 pl-8" },
    { header: "Source", colClass: "w-44", thClass: "px-2.5" },
    { header: "Type", colClass: "w-28", thClass: "px-2.5" },
    { header: "Status", colClass: "w-44", thClass: "px-2.5" },
    { header: "PII", colClass: "w-14", thClass: "px-2.5 text-right" },
    { header: "Updated", colClass: "w-36", thClass: "pr-8 pl-2.5" },
  ] as const;
</script>

{#if visibleDocs.length === 0}
  <div class="flex flex-col items-center justify-center gap-4 px-8 py-16 text-center">
    <div class="flex flex-col items-center gap-1">
      <p class="text-sm font-medium text-ink">No files match these filters</p>
      <p class="max-w-[40ch] text-[13px] text-pretty text-ink-soft">
        Try a different name, source, or type.
      </p>
    </div>
    <button type="button" class="btn-ghost" onclick={onClear}>Clear filters</button>
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
        <tr class="cursor-pointer hover:bg-navy-50/60" onclick={() => onOpen(doc)}>
          <td
            class="truncate border-b border-paper-line/70 py-2.5 pr-2.5 pl-8 font-medium text-ink"
            title={doc.relPath}
          >
            <button
              type="button"
              class="block w-full truncate text-left font-medium text-ink focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-navy-500"
              onclick={(event) => {
                event.stopPropagation();
                onOpen(doc);
              }}>{doc.fileName}</button
            >
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
              <span class="inline-flex items-center gap-2">
                <span class="text-red-700/80" title={doc.error}>Error</span>
                <button
                  type="button"
                  class="btn-ghost !px-1.5 !py-0.5 !text-[11.5px]"
                  aria-label="Try again: {doc.fileName}"
                  onclick={(event) => {
                    event.stopPropagation();
                    onRetry(doc);
                  }}>Try again</button
                >
              </span>
            {/if}
          </td>
          <td
            class="border-b border-paper-line/70 px-2.5 py-2.5 text-right text-ink-soft tabular-nums"
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
