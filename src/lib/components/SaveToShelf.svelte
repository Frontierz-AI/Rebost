<script lang="ts">
  import { api, type SaveFormat, type StoredMessage } from "$lib/api";
  import { t } from "$lib/i18n.svelte";
  import { chatState, notifyInvokeError } from "$lib/stores.svelte";
  import {
    canSaveToShelf,
    savedNotice,
    SAVE_FORMATS,
    SAVE_FORMAT_LABEL_KEYS,
    withSaved,
  } from "$lib/saved-doc";
  import { Check, ExternalLink, FilePlus, Undo2 } from "@lucide/svelte";

  let {
    threadId,
    message,
    shelfId,
    busy = false,
  }: {
    threadId: string;
    message: StoredMessage;
    /** Library Shelf of this conversation, if any. */
    shelfId: string | null;
    busy?: boolean;
  } = $props();

  let format = $state<SaveFormat>("docx");
  let working = $state(false);

  const notice = $derived(savedNotice(message.saved));
  const offer = $derived(canSaveToShelf(message, shelfId, busy));
  const formatId = $derived(`save-format-${message.id}`);

  async function save() {
    if (working) return;
    working = true;
    try {
      const saved = await api.chatSaveMessage(threadId, message.id, format);
      chatState.messages = withSaved(chatState.messages, message.id, saved);
    } catch (error) {
      notifyInvokeError(error);
    } finally {
      working = false;
    }
  }

  async function open() {
    const path = message.saved?.path;
    if (!path) return;
    try {
      await api.openOriginal(path);
    } catch (error) {
      notifyInvokeError(error);
    }
  }

  async function undo() {
    if (working) return;
    working = true;
    try {
      const saved = await api.chatUndoSave(threadId, message.id);
      chatState.messages = withSaved(chatState.messages, message.id, saved);
    } catch (error) {
      notifyInvokeError(error);
    } finally {
      working = false;
    }
  }
</script>

{#if notice}
  <div
    class="flex flex-wrap items-center gap-x-1 gap-y-0.5 text-[12px] {notice.tone === 'error'
      ? 'text-rose-700 dark:text-rose-300'
      : 'text-ink-soft'}"
    role="status"
  >
    {#if notice.tone === "ok"}
      <Check size={13} class="shrink-0 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
    {/if}
    <span class="min-w-0 break-words">{t(notice.key, notice.params)}</span>
    {#if notice.canOpen}
      <button
        type="button"
        class="btn-ghost shrink-0 !px-2 !py-1 !text-[11.5px] whitespace-nowrap"
        onclick={open}
      >
        <ExternalLink size={13} class="shrink-0" aria-hidden="true" />
        {t("chat.saveOpen")}
      </button>
    {/if}
    {#if notice.canUndo}
      <button
        type="button"
        class="btn-ghost shrink-0 !px-2 !py-1 !text-[11.5px] whitespace-nowrap"
        disabled={working}
        onclick={undo}
      >
        <Undo2 size={13} class="shrink-0" aria-hidden="true" />
        {t("chat.saveUndo")}
      </button>
    {/if}
  </div>
{/if}

{#if offer}
  <div class="flex flex-wrap items-center gap-x-1 gap-y-0.5 text-ink-faint">
    <button
      type="button"
      class="btn-ghost shrink-0 !px-2 !py-1 !text-[11.5px] whitespace-nowrap"
      disabled={working}
      title={t("chat.saveToShelfTitle")}
      onclick={save}
    >
      <FilePlus size={13} class="shrink-0" aria-hidden="true" />
      {working ? t("chat.saving") : t("chat.saveToShelf")}
    </button>
    <label class="sr-only" for={formatId}>{t("chat.saveFormat")}</label>
    <span class="select-wrap w-fit">
      <select
        id={formatId}
        class="input select !w-auto !py-0.5 !text-[11.5px]"
        bind:value={format}
        disabled={working}
      >
        {#each SAVE_FORMATS as choice (choice)}
          <option value={choice}>{t(SAVE_FORMAT_LABEL_KEYS[choice])}</option>
        {/each}
      </select>
    </span>
  </div>
{/if}
