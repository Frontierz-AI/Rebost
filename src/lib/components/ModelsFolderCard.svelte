<script lang="ts">
  import { onDestroy } from "svelte";
  import { FolderOpen, HardDrive, TriangleAlert, X } from "@lucide/svelte";
  import {
    api,
    events,
    formatBytes,
    formatTransferBytes,
    userFacingError,
    type ModelsFolderView,
    type ModelsMoveEvent,
  } from "$lib/api";
  import { app, notify, notifyInvokeError, refreshSettings } from "$lib/stores.svelte";
  import { t } from "$lib/i18n.svelte";
  import { confirmAction, confirmDanger } from "$lib/native-dialog";
  import { MOVE_ID, folderLabel, folderName, folderProblemMessage } from "$lib/models-folder";

  let {
    folder,
    freeBytes,
    installedBytes,
    busy = false,
    onChanged,
  }: {
    folder: ModelsFolderView;
    /** Free space on the folder's drive, when known. */
    freeBytes: number | null | undefined;
    /** Size of the installed AIs (what a move carries). */
    installedBytes: number;
    /** A download or an answer holds the AI; choosing waits. */
    busy?: boolean;
    /** Free space and recommendations follow the folder. */
    onChanged?: () => void;
  } = $props();

  let picking = $state(false);
  let moving = $state<ModelsMoveEvent | null>(null);
  let checking = $state(false);
  let error = $state("");
  let notice = $state("");
  let stopListening: (() => void) | null = null;

  onDestroy(() => stopListening?.());

  const working = $derived(picking || !!moving || checking);
  const movePercent = $derived(
    moving && moving.total > 0 ? Math.min(100, Math.round((moving.done / moving.total) * 100)) : 0,
  );

  async function choose() {
    if (working || busy) return;
    error = "";
    notice = "";
    picking = true;
    try {
      const choice = await api.modelsFolderPick();
      if (!choice || choice.unchanged) return;
      const problem = folderProblemMessage(choice);
      if (problem) {
        error = problem;
        return;
      }
      if (choice.moveBytes > 0) {
        const ok = await confirmAction(
          choice.isDefault
            ? t("modelsFolder.moveBackConfirm", { size: formatBytes(choice.moveBytes) })
            : t("modelsFolder.moveConfirm", {
                name: folderName(choice.path),
                size: formatBytes(choice.moveBytes),
              }),
          t("modelsFolder.moveAction"),
        );
        if (!ok) return;
      }
      picking = false;
      await run(() => api.modelsFolderApply(), choice.moveBytes);
    } catch (err) {
      error = userFacingError(err);
    } finally {
      picking = false;
    }
  }

  async function useDefault() {
    if (working || busy) return;
    error = "";
    notice = "";
    if (folder.missing) {
      const ok = await confirmDanger(
        t("modelsFolder.forgetConfirm"),
        t("modelsFolder.forgetAction"),
      );
      if (!ok) return;
      await run(() => api.modelsFolderUseDefault(), 0);
      return;
    }
    if (installedBytes > 0) {
      const ok = await confirmAction(
        t("modelsFolder.moveBackConfirm", { size: formatBytes(installedBytes) }),
        t("modelsFolder.moveAction"),
      );
      if (!ok) return;
    }
    await run(() => api.modelsFolderUseDefault(), installedBytes);
  }

  async function run(action: () => Promise<boolean>, bytes: number) {
    if (bytes > 0) {
      moving = { done: 0, total: bytes };
      stopListening = await events.modelsMove((event) => {
        if (moving) moving = event;
      });
    }
    try {
      const finished = await action();
      if (!finished) notice = t("modelsFolder.cancelled");
      else if (bytes > 0) notify(t("modelsFolder.moved"));
    } catch (err) {
      error = userFacingError(err);
    } finally {
      stopListening?.();
      stopListening = null;
      moving = null;
      await refreshSettings().catch(notifyInvokeError);
      onChanged?.();
    }
  }

  async function recheck() {
    if (working) return;
    error = "";
    notice = "";
    checking = true;
    try {
      const found = await api.modelsFolderRecheck();
      if (!found) notice = t("modelsFolder.stillMissing");
      await refreshSettings();
      app.engine = await api.engineStatus();
      onChanged?.();
    } catch (err) {
      error = userFacingError(err);
    } finally {
      checking = false;
    }
  }
</script>

<section class="card mb-6 px-4 py-5 min-[900px]:px-6" aria-labelledby="models-folder-heading">
  <h2 id="models-folder-heading" class="mb-1 text-[15px] font-semibold text-ink">
    {t("modelsFolder.heading")}
  </h2>
  <p class="mb-4 text-[12.5px] leading-snug text-ink-soft">{t("modelsFolder.help")}</p>

  {#if moving}
    <div
      class="flex flex-col gap-2 rounded-xl border border-paper-line bg-paper-soft px-4 py-3"
      role="group"
      aria-label={t("modelsFolder.moving")}
    >
      <div class="flex items-center justify-between gap-3 text-[13px]">
        <p class="font-medium text-ink">{t("modelsFolder.moving")}</p>
        <p class="text-ink-soft tabular-nums">
          {formatTransferBytes(moving.done)} / {formatTransferBytes(moving.total)}
        </p>
      </div>
      <div
        class="h-1.5 overflow-hidden rounded-full bg-navy-100 dark:bg-white/10"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={movePercent}
        aria-label={t("modelsFolder.moving")}
      >
        <div
          class="progress-bar h-full w-full bg-navy-500"
          style="transform: scaleX({movePercent / 100})"
        ></div>
      </div>
      <p class="text-[11.5px] text-ink-faint">{t("modelsFolder.movingNote")}</p>
      <div>
        <button
          type="button"
          class="btn-ghost -ml-2 !px-2 !py-1 !text-[11.5px]"
          onclick={() => void api.downloadCancel(MOVE_ID).catch(notifyInvokeError)}
        >
          <X size={11} aria-hidden="true" />
          {t("downloads.cancel")}
        </button>
      </div>
    </div>
  {:else if folder.missing}
    <div
      class="rounded-xl border border-amber-350/70 bg-amber-350/10 px-4 py-3 dark:border-amber-350/30"
    >
      <div class="flex items-start gap-3">
        <TriangleAlert
          size={18}
          class="mt-px shrink-0 text-amber-550 dark:text-amber-350"
          aria-hidden="true"
        />
        <div class="min-w-0 flex-1">
          <p class="text-[13.5px] font-semibold text-ink">{t("modelsFolder.missingTitle")}</p>
          <p class="truncate text-[11.5px] text-ink-soft" title={folder.path}>{folder.path}</p>
          <p class="mt-1 text-[12.5px] leading-snug text-ink-soft">
            {t("modelsFolder.missingHelp")}
          </p>
        </div>
      </div>
      <div class="mt-3 flex flex-wrap items-center gap-2 pl-[1.875rem]">
        <button type="button" class="btn-outline" onclick={recheck} disabled={working}>
          {t("modelsFolder.tryAgain")}
        </button>
        <button type="button" class="btn-ghost" onclick={choose} disabled={working || busy}>
          {t("modelsFolder.chooseAnother")}
        </button>
        <button type="button" class="btn-ghost" onclick={useDefault} disabled={working || busy}>
          {t("modelsFolder.useDefault")}
        </button>
      </div>
    </div>
  {:else}
    <div
      class="flex flex-wrap items-center gap-3 rounded-xl border border-paper-line bg-paper-soft/50 py-3 pr-2 pl-4"
    >
      <HardDrive size={18} class="shrink-0 text-navy-600 dark:text-navy-400" aria-hidden="true" />
      <div class="min-w-0 flex-1 basis-40">
        <p class="truncate text-[13.5px] font-semibold text-ink">{folderLabel(folder)}</p>
        <p class="truncate text-[11.5px] text-ink-soft tabular-nums" title={folder.path}>
          {[
            freeBytes ? t("modelsFolder.free", { size: formatBytes(freeBytes) }) : "",
            folder.custom ? folder.path : "",
          ]
            .filter(Boolean)
            .join(" · ") || folder.path}
        </p>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <button
          type="button"
          class="btn-ghost relative !p-1.5"
          aria-label={t("settings.openAiFolder")}
          title={t("settings.openAiFolder")}
          onclick={() => void api.modelReveal().catch(notifyInvokeError)}
        >
          <span
            class="absolute top-1/2 left-1/2 size-[max(100%,3rem)] -translate-1/2 pointer-fine:hidden"
            aria-hidden="true"
          ></span>
          <FolderOpen size={15} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="btn-outline !h-8 !px-3.5 !text-[12px]"
          onclick={choose}
          disabled={working || busy}
        >
          {t("modelsFolder.change")}
        </button>
      </div>
    </div>
    {#if folder.custom}
      <button
        type="button"
        class="btn-ghost mt-2 -ml-3 !text-[12px]"
        onclick={useDefault}
        disabled={working || busy}
      >
        {t("modelsFolder.useDefault")}
      </button>
    {/if}
  {/if}

  {#if error}
    <p role="alert" class="mt-2 text-sm text-red-700 dark:text-red-400">{error}</p>
  {:else if notice}
    <p role="status" class="mt-2 text-sm text-ink-soft">{notice}</p>
  {/if}
</section>
