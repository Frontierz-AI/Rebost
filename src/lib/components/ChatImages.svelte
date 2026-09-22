<script lang="ts">
  import { api, type ChatImage } from "$lib/api";
  import { catalogsByLocale, t } from "$lib/i18n.svelte";
  import { focusTrap } from "$lib/focus-trap";
  import { notifyInvokeError } from "$lib/stores.svelte";
  import { ImageOff, X } from "@lucide/svelte";
  import ChatImagePlaceholder from "./ChatImagePlaceholder.svelte";

  let {
    threadId,
    images,
    onRemove,
    pending = 0,
  }: {
    threadId: string;
    images: ChatImage[];
    onRemove?: (image: ChatImage) => void;
    pending?: number;
  } = $props();
  let previews = $state<Record<string, string | null>>({});
  let loaded = $state<Record<string, boolean>>({});
  let opened = $state<{ image: ChatImage; url: string } | null>(null);
  const pastedImageNames = new Set(
    Object.values(catalogsByLocale).map((catalog) => catalog.images.pasted),
  );

  function filename(image: ChatImage): string | undefined {
    return pastedImageNames.has(image.name) ? undefined : image.name || undefined;
  }

  function showPreview(dialog: HTMLDialogElement) {
    const focus = focusTrap(dialog);
    // The top layer escapes the chat scroll mask and makes the rest of the app inert.
    dialog.showModal();
    return {
      destroy() {
        dialog.close();
        focus.destroy();
      },
    };
  }

  $effect(() => {
    const thread = threadId;
    const pending = images;
    let cancelled = false;
    void Promise.all(
      pending.map(async (image) => {
        try {
          const url = await api.chatImageRead(thread, image.id);
          if (!cancelled) previews[image.id] = url;
        } catch {
          if (!cancelled) previews[image.id] = null;
        }
      }),
    );
    return () => {
      cancelled = true;
    };
  });

  async function expand(image: ChatImage) {
    try {
      const thread = threadId;
      const url = await api.chatImageRead(thread, image.id, false);
      if (thread === threadId) opened = { image, url };
    } catch (error) {
      notifyInvokeError(error);
    }
  }
</script>

<ul role="list" class="flex flex-wrap gap-3 py-2" aria-label={t("images.attached")}>
  {#each images as image (image.id)}
    <li class="relative min-w-0 {onRemove ? 'w-24 sm:w-28' : 'w-36 sm:w-44'}">
      <button
        type="button"
        class="group w-full rounded-xl text-left focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-500"
        onclick={(event) => {
          event.currentTarget.focus();
          void expand(image);
        }}
        disabled={!previews[image.id] || !loaded[image.id]}
        aria-busy={previews[image.id] !== null && !loaded[image.id]}
        title={previews[image.id] === null ? t("images.missingShort") : filename(image)}
        aria-label={previews[image.id] === null
          ? t("images.missingShort")
          : t("images.preview", { name: image.name })}
      >
        <div
          class="relative flex aspect-4/3 w-full items-center justify-center overflow-hidden rounded-xl bg-paper-soft outline-1 -outline-offset-1 outline-ink/10 group-enabled:group-hover:outline-navy-500"
        >
          {#if previews[image.id]}
            <img
              src={previews[image.id]}
              alt=""
              class="absolute inset-0 size-full object-contain {loaded[image.id]
                ? ''
                : 'opacity-0'}"
              onload={() => (loaded[image.id] = true)}
              onerror={() => (previews[image.id] = null)}
            />
          {:else if previews[image.id] === null}
            <ImageOff size={16} class="shrink-0 text-ink-soft" aria-hidden="true" />
          {/if}
          {#if previews[image.id] !== null && !loaded[image.id]}
            <ChatImagePlaceholder />
          {/if}
        </div>
      </button>
      {#if onRemove}
        <button
          type="button"
          class="btn-ghost absolute -top-1 -right-1 size-7 bg-surface !p-1 text-ink outline-1 outline-ink/10 hover:bg-paper-soft"
          onclick={() => onRemove?.(image)}
          title={filename(image) ? t("images.remove", { name: image.name }) : undefined}
          aria-label={t("images.remove", { name: image.name })}
        >
          <X size={16} class="shrink-0" aria-hidden="true" />
          <span
            class="absolute top-1/2 left-1/2 size-[max(100%,3rem)] -translate-1/2 pointer-fine:hidden"
            aria-hidden="true"
          ></span>
        </button>
      {/if}
    </li>
  {/each}
  {#each Array(pending) as _}
    <li class="min-w-0 {onRemove ? 'w-24 sm:w-28' : 'w-36 sm:w-44'}">
      <ChatImagePlaceholder />
    </li>
  {/each}
</ul>

{#if pending}
  <p role="status" class="sr-only">{t("images.preparing")}</p>
{/if}

{#if opened}
  <dialog
    class="fixed inset-0 m-0 size-full max-h-none max-w-none overflow-hidden border-0 bg-transparent p-4 text-ink backdrop:bg-navy-950/60 open:flex open:items-center open:justify-center sm:p-8"
    aria-label={opened.image.name}
    use:showPreview
    onclick={(event) => {
      if (event.target === event.currentTarget) opened = null;
    }}
    oncancel={(event) => {
      event.preventDefault();
      opened = null;
    }}
    onclose={() => (opened = null)}
  >
    <div
      class="flex max-h-full w-full max-w-5xl flex-col gap-3 rounded-2xl bg-surface p-3 text-ink sm:p-4"
    >
      <div class="flex min-w-0 items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm text-ink-soft tabular-nums">
            {opened.image.width} × {opened.image.height}
          </p>
        </div>
        <button
          type="button"
          class="btn-ghost btn-icon shrink-0"
          onclick={() => (opened = null)}
          aria-label={t("images.closePreview")}><X size={16} aria-hidden="true" /></button
        >
      </div>
      <img
        src={opened.url}
        alt={opened.image.name}
        title={filename(opened.image)}
        class="min-h-0 w-full flex-1 rounded-lg object-contain"
      />
    </div>
  </dialog>
{/if}
