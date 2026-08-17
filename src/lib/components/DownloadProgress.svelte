<script lang="ts">
  import { api, downloadHeadline, formatTransferBytes, type DownloadEvent } from "$lib/api";
  import { X } from "@lucide/svelte";

  let {
    download,
    cancelable = false,
    note,
  }: {
    download: DownloadEvent;
    cancelable?: boolean;
    note?: string;
  } = $props();

  function pct(received?: number, total?: number | null): number {
    if (!received || !total) return 0;
    return Math.min(100, Math.round((received / total) * 100));
  }

  const percent = $derived(download.total ? pct(download.received, download.total) : 0);
  const determinate = $derived(!!download.total && download.total > 0);
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-center justify-between text-[13px]">
    <p class="font-medium text-ink">{downloadHeadline(download)}</p>
    <p class="text-ink-soft tabular-nums">
      {formatTransferBytes(download.received)}{download.total
        ? ` / ${formatTransferBytes(download.total)}`
        : ""}
    </p>
  </div>
  <div
    class="h-1.5 overflow-hidden rounded-full bg-navy-100 dark:bg-white/10"
    role="progressbar"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={determinate ? percent : undefined}
    aria-label={downloadHeadline(download)}
  >
    {#if determinate}
      <div
        class="progress-bar h-full w-full bg-amber-450"
        style="transform: scaleX({percent / 100})"
      ></div>
    {:else}
      <div class="progress-indeterminate" aria-hidden="true"></div>
    {/if}
  </div>
  {#if note}
    <p class="text-[11.5px] text-ink-faint">{note}</p>
  {/if}
  {#if (download.kind === "model" && download.phase === "verifying") || cancelable}
    <div class="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-1">
      {#if download.kind === "model" && download.phase === "verifying"}
        <button
          type="button"
          class="rounded-sm text-[11.5px] text-ink-soft underline decoration-navy-200 underline-offset-2 hover:text-ink hover:decoration-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-400"
          onclick={() => api.downloadSkipVerify(download.id)}
        >
          Skip the check and use the file
        </button>
      {/if}
      {#if cancelable}
        <button
          type="button"
          class="btn-ghost !px-2 !py-1 !text-[11.5px]"
          onclick={() => api.downloadCancel(download.id)}
        >
          <X size={11} aria-hidden="true" /> Cancel
        </button>
      {/if}
    </div>
  {/if}
</div>
