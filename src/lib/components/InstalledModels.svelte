<script lang="ts">
  import type { Snippet } from "svelte";
  import { api, formatBytes, type InstalledModel } from "$lib/api";
  import { app, notifyInvokeError, refreshSettings } from "$lib/stores.svelte";
  import { t } from "$lib/i18n.svelte";
  import { confirmDanger } from "$lib/native-dialog";
  import { popupNativeMenu, type NativeMenuEntry } from "$lib/native-menu";
  import { BadgeCheck, Ellipsis, FolderOpen } from "@lucide/svelte";

  let {
    active,
    others,
    busy = false,
    children,
  }: {
    active: InstalledModel | null | undefined;
    others: InstalledModel[];
    /** A model download holds the install lock; switching and removing wait for it. */
    busy?: boolean;
    /** Rendered under the AI in use (image support line). */
    children?: Snippet;
  } = $props();

  let switchingTo = $state<string | null>(null);
  let removing = $state<string | null>(null);

  const statusLabel = $derived(
    app.engine.state === "ready"
      ? t("settings.engineReady")
      : app.engine.state === "starting"
        ? t("settings.engineWarming")
        : t("settings.engineIdle"),
  );

  function footprint(model: InstalledModel) {
    return model.sizeBytes + (model.projector?.sizeBytes ?? 0);
  }

  const installed = $derived(active ? [active, ...others] : others);
  const totalBytes = $derived.by(() => {
    const projectors = new Map<string, number>();
    let bytes = 0;
    for (const model of installed) {
      bytes += model.sizeBytes;
      if (model.projector) projectors.set(model.projector.file, model.projector.sizeBytes);
    }
    for (const size of projectors.values()) bytes += size;
    return bytes;
  });

  function details(model: InstalledModel) {
    return `${formatBytes(footprint(model))}${model.license ? ` · ${model.license}` : ""}`;
  }

  async function use(model: InstalledModel) {
    if (switchingTo || busy) return;
    switchingTo = model.file;
    try {
      await api.modelUse(model.file);
    } catch (error) {
      notifyInvokeError(error);
    } finally {
      switchingTo = null;
      await refreshSettings().catch(notifyInvokeError);
    }
  }

  async function remove(model: InstalledModel) {
    if (removing || busy) return;
    const ok = await confirmDanger(
      t("settings.removeAiConfirm", { name: model.name, size: formatBytes(footprint(model)) }),
      t("settings.removeAiAction"),
    );
    if (!ok) return;
    removing = model.file;
    try {
      await api.modelRemove(model.file);
    } catch (error) {
      notifyInvokeError(error);
    } finally {
      removing = null;
      await refreshSettings().catch(notifyInvokeError);
    }
  }

  function catalogHost(model: InstalledModel) {
    return model.source === "ollama" ? t("explore.ollama") : t("explore.huggingface");
  }

  function openMenu(model: InstalledModel, inUse: boolean) {
    const entries: NativeMenuEntry[] = [
      {
        kind: "item",
        text: t("documents.showInFolder"),
        action: () => void api.modelReveal(model.file).catch(notifyInvokeError),
      },
      {
        kind: "item",
        text: t("explore.moreOn", { host: catalogHost(model) }),
        action: () =>
          void api.openModelPage(model.source, model.reference).catch(notifyInvokeError),
      },
    ];
    if (!inUse) {
      entries.push(
        { kind: "separator" },
        {
          kind: "item",
          text: t("settings.removeAi"),
          enabled: !busy && !switchingTo,
          action: () => void remove(model),
        },
      );
    }
    void popupNativeMenu(entries).catch(() => {});
  }
</script>

{#if active}
  {@const model = active}
  <div
    class="flex flex-wrap items-center gap-3 rounded-xl border border-paper-line bg-paper-soft/50 py-3 pr-2 pl-4"
  >
    <BadgeCheck size={18} class="shrink-0 text-navy-600 dark:text-navy-400" />
    <div class="min-w-0 flex-1">
      <p class="text-[13.5px] font-semibold text-ink">{model.name}</p>
      <p class="text-[11.5px] text-ink-soft tabular-nums">
        {details(model)} · {t("settings.installedHere")}
      </p>
    </div>
    <span
      class="rounded-full px-2 py-1 text-[10.5px] font-semibold
      {app.engine.state === 'ready'
        ? 'bg-ready text-ready-ink dark:bg-navy-200/20 dark:text-navy-200'
        : app.engine.state === 'starting'
          ? 'bg-amber-350/50 text-amber-550'
          : 'bg-paper-soft text-ink-faint'}"
    >
      {statusLabel}
    </span>
    <button
      type="button"
      class="btn-ghost !p-1.5"
      aria-label={t("settings.aiActions", { name: model.name })}
      onclick={() => openMenu(model, true)}
    >
      <Ellipsis size={15} aria-hidden="true" />
    </button>
  </div>
{/if}

{@render children?.()}

{#if others.length > 0}
  <h3 class="label mt-5">{t("settings.alsoInstalled")}</h3>
  <ul
    class="mt-2 divide-y divide-paper-line overflow-hidden rounded-xl border border-paper-line"
    role="list"
  >
    {#each others as model (model.file)}
      <li class="flex items-center gap-3 py-2.5 pr-2 pl-4">
        <div class="min-w-0 flex-1">
          <p class="truncate text-[13.5px] font-semibold text-ink">{model.name}</p>
          <p class="text-[11.5px] text-ink-soft tabular-nums">{details(model)}</p>
        </div>
        <button
          type="button"
          class="btn-outline !h-8 shrink-0 !px-3.5 !text-[12px]"
          disabled={busy || !!switchingTo || removing === model.file}
          aria-label={t("settings.useAiNamed", { name: model.name })}
          onclick={() => use(model)}
        >
          {switchingTo === model.file ? t("settings.switchingAi") : t("settings.useAi")}
        </button>
        <button
          type="button"
          class="btn-ghost !p-1.5"
          aria-label={t("settings.aiActions", { name: model.name })}
          onclick={() => openMenu(model, false)}
        >
          <Ellipsis size={15} aria-hidden="true" />
        </button>
      </li>
    {/each}
  </ul>
{/if}

{#if installed.length > 0}
  <div class="mt-3 flex flex-wrap items-center justify-between gap-2">
    <p class="text-[12px] text-ink-soft tabular-nums">
      {installed.length === 1
        ? t("settings.installedOne", { size: formatBytes(totalBytes) })
        : t("settings.installedMany", {
            count: installed.length,
            size: formatBytes(totalBytes),
          })}
    </p>
    <button
      type="button"
      class="btn-ghost -mr-3"
      onclick={() => void api.modelReveal().catch(notifyInvokeError)}
    >
      <FolderOpen size={13} class="shrink-0" aria-hidden="true" />
      {t("settings.openAiFolder")}
    </button>
  </div>
{/if}
