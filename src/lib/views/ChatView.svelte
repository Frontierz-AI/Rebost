<script lang="ts">
  import { api, type SourcePassage, type StoredMessage } from "$lib/api";
  import { importIntoChat } from "$lib/chat-import";
  import { listenFileDrop } from "$lib/files";
  import {
    app,
    chatState,
    newConversation,
    notifyInvokeError,
    rememberPreferredShelf,
    openThread,
    loadOlderMessages,
    refreshThreads,
  } from "$lib/stores.svelte";
  import { clipChars, PROMPT_MAX_CHARS } from "$lib/text-cap";
  import {
    isOutboundPlaceholder,
    outboundPlaceholderId,
    pendingForThread,
    threadIsBusy,
  } from "$lib/chat-reducer";
  import Markdown from "$lib/components/Markdown.svelte";
  import CopyActions from "$lib/components/CopyActions.svelte";
  import SourcePanel from "$lib/components/SourcePanel.svelte";
  import ChatThreadList from "$lib/components/ChatThreadList.svelte";
  import ChatThreadHeader from "$lib/components/ChatThreadHeader.svelte";
  import ChatComposer from "$lib/components/ChatComposer.svelte";
  import ChatEmptyState from "$lib/components/ChatEmptyState.svelte";
  import ThinkingStatus from "$lib/components/ThinkingStatus.svelte";
  import ThinkingPanel from "$lib/components/ThinkingPanel.svelte";
  import { confirmDanger } from "$lib/native-dialog";
  import { tick } from "svelte";

  let composerEl = $state<HTMLTextAreaElement | null>(null);
  let scrollEl = $state<HTMLDivElement | null>(null);
  let openSource = $state<SourcePassage | null>(null);
  let autoScroll = $state(true);
  let openThinking = $state<Record<string, boolean>>({});
  let dropActive = $state(false);

  const activeThread = $derived(app.threads.find((t) => t.id === chatState.activeThreadId) ?? null);
  const hasModel = $derived(!!app.settings?.activeModel);
  const activePending = $derived(pendingForThread(chatState.pending, chatState.activeThreadId));
  const warming = $derived(activePending?.phase === "queued" && app.engine.state !== "ready");
  const pendingStage = $derived(
    activePending?.stage ?? (activePending?.phase === "streaming" ? "thinking" : null),
  );
  const pendingHasShelf = $derived(
    !!(chatState.selectedShelfId || chatState.uploadShelf || activeThread?.uploadShelfId),
  );
  const pendingHasHistory = $derived(chatState.messages.length > 1);
  const generating = $derived(
    threadIsBusy(chatState.pending, chatState.outbound, chatState.activeThreadId),
  );
  const empty = $derived(chatState.messages.length === 0 && !generating);

  $effect(() => {
    if (!activeThread) return;
    const shelfId = activeThread.shelfId ?? null;
    const uploadId = activeThread.uploadShelfId ?? null;
    chatState.selectedShelfId = shelfId && shelfId !== uploadId ? shelfId : null;
  });

  $effect(() => {
    return listenFileDrop({
      onOver: (active) => (dropActive = active),
      onDrop: (paths) => {
        void importIntoChat(paths);
      },
    });
  });

  const dropHint = "Drop files to use them in this conversation";

  async function chooseShelf(shelfId: string | null) {
    rememberPreferredShelf(shelfId);
    chatState.selectedShelfId = shelfId;
    if (!chatState.activeThreadId) return;
    try {
      await api.threadSetShelf(chatState.activeThreadId, shelfId);
      await refreshThreads();
    } catch (error) {
      notifyInvokeError(error);
    }
  }

  $effect(() => {
    if (chatState.draftFocus > 0 && composerEl) {
      void chatState.draftFocus;
      tick().then(() => {
        autoresize();
        composerEl?.focus();
        const marker = chatState.draft.indexOf("«");
        if (marker >= 0) {
          const end = chatState.draft.indexOf("»", marker);
          composerEl?.setSelectionRange(marker, end >= 0 ? end + 1 : marker);
        } else {
          const end = chatState.draft.length;
          composerEl?.setSelectionRange(end, end);
        }
      });
    }
  });

  function markOutbound(threadId: string) {
    chatState.outbound = { ...chatState.outbound, [threadId]: true };
  }

  function clearOutbound(threadId: string) {
    if (!chatState.outbound[threadId]) return;
    const next = { ...chatState.outbound };
    delete next[threadId];
    chatState.outbound = next;
  }

  function setPlaceholderPending(threadId: string) {
    chatState.pending = {
      ...chatState.pending,
      [threadId]: {
        messageId: outboundPlaceholderId(threadId),
        threadId,
        text: "",
        thinking: "",
        phase: "queued",
      },
    };
  }

  function dropPending(threadId: string) {
    if (!chatState.pending[threadId]) return;
    const next = { ...chatState.pending };
    delete next[threadId];
    chatState.pending = next;
  }

  function clearCancelWhenQueued(threadId: string) {
    if (!chatState.cancelWhenQueued[threadId]) return;
    const next = { ...chatState.cancelWhenQueued };
    delete next[threadId];
    chatState.cancelWhenQueued = next;
  }

  async function send() {
    const text = clipChars(chatState.draft.trim(), PROMPT_MAX_CHARS);
    if (!text || generating || !hasModel) return;
    let outboundKey = chatState.activeThreadId ?? "new";
    markOutbound(outboundKey);
    if (chatState.activeThreadId) {
      setPlaceholderPending(chatState.activeThreadId);
    }
    try {
      let threadId = chatState.activeThreadId;
      if (!threadId) {
        const thread = await api.threadCreate(chatState.selectedShelfId);
        threadId = thread.id;
        await refreshThreads();
        chatState.activeThreadId = threadId;
        chatState.messages = [];
        chatState.hasOlder = false;
        if (chatState.outbound["new"]) {
          clearOutbound("new");
          markOutbound(threadId);
        }
        if (chatState.cancelWhenQueued["new"]) {
          const next = { ...chatState.cancelWhenQueued };
          delete next["new"];
          next[threadId] = true;
          chatState.cancelWhenQueued = next;
        }
        outboundKey = threadId;
        setPlaceholderPending(threadId);
      }
      const optimistic: StoredMessage = {
        id: `local-${Date.now()}`,
        role: "user",
        text,
        ts: new Date().toISOString(),
        shelfId: chatState.selectedShelfId,
        sources: [],
        status: "done",
      };
      chatState.messages = [...chatState.messages, optimistic];
      chatState.draft = "";
      autoScroll = true;
      await tick();
      autoresize();
      scrollToBottom();
      await api.chatSend(threadId, text, chatState.selectedShelfId);
    } catch (error) {
      clearOutbound(outboundKey);
      dropPending(outboundKey);
      clearCancelWhenQueued(outboundKey);
      clearCancelWhenQueued("new");
      notifyInvokeError(error);
    }
  }

  function stop() {
    const threadId = chatState.activeThreadId ?? "new";
    const inFlight = pendingForThread(chatState.pending, chatState.activeThreadId);
    if (inFlight && !isOutboundPlaceholder(inFlight.messageId)) {
      api.chatCancel(inFlight.messageId).catch(notifyInvokeError);
      return;
    }
    chatState.cancelWhenQueued = { ...chatState.cancelWhenQueued, [threadId]: true };
  }

  function autoresize() {
    if (!composerEl) return;
    composerEl.style.height = "auto";
    composerEl.style.height = `${Math.min(composerEl.scrollHeight, 180)}px`;
  }

  function scrollToBottom() {
    if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
  }

  function onScroll() {
    if (!scrollEl) return;
    autoScroll = scrollEl.scrollTop + scrollEl.clientHeight >= scrollEl.scrollHeight - 60;
  }

  async function readMore() {
    const el = scrollEl;
    const prevHeight = el?.scrollHeight ?? 0;
    const prevTop = el?.scrollTop ?? 0;
    try {
      await loadOlderMessages();
    } catch (error) {
      notifyInvokeError(error);
      return;
    }
    await tick();
    if (el) {
      el.scrollTop = prevTop + (el.scrollHeight - prevHeight);
    }
  }

  $effect(() => {
    void activePending?.text;
    void activePending?.thinking;
    void chatState.messages.length;
    if (autoScroll) {
      tick().then(() => autoScroll && scrollToBottom());
    }
  });

  async function removeThread(threadId: string) {
    const ok = await confirmDanger("Delete this conversation? This cannot be undone.", "Delete");
    if (!ok) return;
    try {
      await api.threadDelete(threadId);
      if (chatState.activeThreadId === threadId) {
        chatState.activeThreadId = null;
        chatState.messages = [];
        chatState.uploadShelf = null;
        chatState.hasOlder = false;
      }
      const inFlight = chatState.pending[threadId];
      if (inFlight && !isOutboundPlaceholder(inFlight.messageId)) {
        api.chatCancel(inFlight.messageId).catch(notifyInvokeError);
      } else if (chatState.outbound[threadId]) {
        chatState.cancelWhenQueued = { ...chatState.cancelWhenQueued, [threadId]: true };
      }
      dropPending(threadId);
      clearOutbound(threadId);
      await refreshThreads();
    } catch (error) {
      notifyInvokeError(error);
    }
  }

  async function renameThread(threadId: string, title: string) {
    try {
      await api.threadRename(threadId, title);
      await refreshThreads();
    } catch (error) {
      notifyInvokeError(error);
    }
  }

  async function exportThread(threadId: string) {
    try {
      await api.threadExport(threadId);
    } catch (error) {
      notifyInvokeError(error);
    }
  }
</script>

<div class="relative flex h-full min-h-0">
  {#if dropActive}
    <div
      class="pointer-events-none absolute inset-3 z-30 flex items-center justify-center rounded-2xl border-2 border-dashed border-amber-450 bg-amber-350/20"
    >
      <p class="rounded-xl bg-navy-900 px-4 py-2 text-[13.5px] font-medium text-white shadow-pop">
        {dropHint}
      </p>
    </div>
  {/if}

  <ChatThreadList
    threads={app.threads}
    activeThreadId={chatState.activeThreadId}
    shelves={app.shelves}
    onOpen={(id) => openThread(id).catch(notifyInvokeError)}
    onNew={newConversation}
    onRemove={removeThread}
    onRename={renameThread}
    onExport={exportThread}
  />

  <section class="flex min-w-0 flex-1 flex-col">
    <div
      bind:this={scrollEl}
      onscroll={onScroll}
      class="min-h-0 flex-1 overflow-y-auto [mask-image:linear-gradient(to_bottom,black_0%,black_calc(100%-25px),transparent_100%)]"
    >
      {#if empty}
        <ChatEmptyState />
      {:else}
        <div class="mx-auto flex max-w-[760px] flex-col gap-4 px-6 pt-4 pb-6">
          {#if activeThread}
            {@const thread = activeThread}
            <ChatThreadHeader
              {thread}
              shelves={app.shelves}
              messageCount={Math.max(thread.messageCount, chatState.messages.length)}
              onDownload={() => void exportThread(thread.id)}
            />
          {/if}
          {#if chatState.hasOlder}
            <div class="flex justify-center">
              <button
                type="button"
                class="btn-outline !py-1.5 !text-[12.5px]"
                onclick={() => void readMore()}
                disabled={chatState.loadingOlder}
                aria-label="Read earlier messages"
              >
                Read more
              </button>
            </div>
          {/if}
          {#each chatState.messages as message (message.id)}
            {#if message.role === "user"}
              <div class="flex justify-end">
                <div
                  class="max-w-[85%] cursor-text rounded-2xl rounded-br-md bg-navy-900 px-4 py-2.5 text-[13.8px] leading-relaxed whitespace-pre-wrap text-white select-text"
                >
                  {message.text}
                </div>
              </div>
            {:else}
              <div class="group flex flex-col gap-1.5">
                <div
                  class="max-w-[92%] rounded-2xl rounded-bl-md border border-paper-line bg-surface px-4 py-3 shadow-card dark:shadow-none"
                >
                  <ThinkingPanel
                    id={message.id}
                    open={!!openThinking[message.id]}
                    onToggle={() =>
                      (openThinking = {
                        ...openThinking,
                        [message.id]: !openThinking[message.id],
                      })}
                    thinking={message.thinking}
                    activity={message.activity}
                  />
                  <Markdown
                    text={message.text}
                    sources={message.sources}
                    onCite={(s) => (openSource = s)}
                  />
                  {#if message.sources.length > 0}
                    <div class="mt-2.5 flex flex-wrap gap-1.5 border-t border-paper-line pt-2.5">
                      {#each message.sources as source (source.sid)}
                        <button
                          type="button"
                          class="chip border border-navy-200 bg-navy-50 text-navy-700 hover:border-amber-450 hover:bg-amber-350/60 dark:border-white/10 dark:bg-white/8 dark:text-navy-100"
                          onclick={() => (openSource = source)}
                        >
                          <span class="font-bold">{source.sid}</span>
                          <span class="max-w-[220px] truncate font-normal">{source.title}</span>
                          {#if source.pageStart}<span class="text-navy-400 dark:text-navy-300"
                              >p. {source.pageStart}</span
                            >{/if}
                        </button>
                      {/each}
                    </div>
                  {/if}
                  {#if message.status === "stopped"}
                    <p class="mt-1.5 text-[11px] text-ink-faint italic">Stopped.</p>
                  {/if}
                </div>
                <CopyActions text={message.text} subtle />
              </div>
            {/if}
          {/each}

          {#if activePending}
            <div
              class="flex flex-col gap-1.5"
              aria-live="polite"
              aria-atomic="false"
              aria-busy="true"
            >
              <div
                class="max-w-[92%] rounded-2xl rounded-bl-md border border-paper-line bg-surface px-4 py-3 shadow-card dark:shadow-none"
              >
                {#if !activePending.text}
                  <ThinkingPanel
                    id="pending"
                    live
                    open={!!openThinking.pending}
                    onToggle={() =>
                      (openThinking = { ...openThinking, pending: !openThinking.pending })}
                    thinking={activePending.thinking}
                    activity={activePending.activity}
                  >
                    {#snippet lead()}
                      <span
                        class="inline-block size-2 shrink-0 animate-pulse rounded-full bg-amber-450 motion-reduce:animate-none"
                      ></span>
                      <span class="min-w-0 grow">
                        {#key activePending.threadId}
                          <ThinkingStatus
                            {warming}
                            stage={pendingStage}
                            hasShelf={pendingHasShelf}
                            hasHistory={pendingHasHistory}
                            file={activePending.file}
                          />
                        {/key}
                      </span>
                    {/snippet}
                  </ThinkingPanel>
                {:else}
                  <ThinkingPanel
                    id="pending"
                    live
                    open={!!openThinking.pending}
                    onToggle={() =>
                      (openThinking = { ...openThinking, pending: !openThinking.pending })}
                    thinking={activePending.thinking}
                    activity={activePending.activity}
                  />
                  <Markdown text={activePending.text} streaming />
                {/if}
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <ChatComposer
      bind:composerEl
      {hasModel}
      {generating}
      onSend={send}
      onStop={stop}
      onChooseShelf={chooseShelf}
      onAutoResize={autoresize}
    />
  </section>
</div>

{#if openSource}
  <SourcePanel source={openSource} onClose={() => (openSource = null)} />
{/if}
