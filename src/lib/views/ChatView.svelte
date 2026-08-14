<script lang="ts">
  import { api, type SourcePassage, type StoredMessage } from "$lib/api";
  import { app, chatState, openThread, refreshThreads, notify } from "$lib/stores.svelte";
  import Markdown from "$lib/components/Markdown.svelte";
  import CopyActions from "$lib/components/CopyActions.svelte";
  import SourcePanel from "$lib/components/SourcePanel.svelte";
  import ChatThreadList from "$lib/components/ChatThreadList.svelte";
  import ChatComposer from "$lib/components/ChatComposer.svelte";
  import { ChevronDown } from "@lucide/svelte";
  import { tick } from "svelte";
  import mark from "../../assets/R.webp";

  let composerEl = $state<HTMLTextAreaElement | null>(null);
  let scrollEl = $state<HTMLDivElement | null>(null);
  let openSource = $state<SourcePassage | null>(null);
  let shelfMenuOpen = $state(false);
  let selectedShelfId = $state<string | null>(null);
  let autoScroll = $state(true);
  let openThinking = $state<Record<string, boolean>>({});

  const activeThread = $derived(app.threads.find((t) => t.id === chatState.activeThreadId) ?? null);
  const hasModel = $derived(!!app.settings?.activeModel);
  const warming = $derived(chatState.pending?.phase === "queued" && app.engine.state !== "ready");
  const generating = $derived(chatState.pending !== null);

  $effect(() => {
    if (activeThread) {
      selectedShelfId = activeThread.shelfId ?? null;
    } else if (app.shelves.length === 1) {
      selectedShelfId = app.shelves[0]!.id;
    }
  });

  const selectedShelf = $derived(app.shelves.find((s) => s.id === selectedShelfId) ?? null);

  async function newThread() {
    const initialShelf = app.shelves.length === 1 ? app.shelves[0]!.id : null;
    const thread = await api.threadCreate(initialShelf);
    await refreshThreads();
    await openThread(thread.id);
  }

  async function chooseShelf(shelfId: string | null) {
    selectedShelfId = shelfId;
    shelfMenuOpen = false;
    if (chatState.activeThreadId) {
      await api.threadSetShelf(chatState.activeThreadId, shelfId);
      await refreshThreads();
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
        }
      });
    }
  });

  async function send() {
    const text = chatState.draft.trim();
    if (!text || generating) return;
    if (!hasModel) {
      notify("Install a model first. Settings has a one-click install.");
      return;
    }
    let threadId = chatState.activeThreadId;
    if (!threadId) {
      const thread = await api.threadCreate(selectedShelfId);
      threadId = thread.id;
      await refreshThreads();
      chatState.activeThreadId = threadId;
      chatState.messages = [];
    }
    const optimistic: StoredMessage = {
      id: `local-${Date.now()}`,
      role: "user",
      text,
      ts: new Date().toISOString(),
      shelfId: selectedShelfId,
      sources: [],
      status: "done",
    };
    chatState.messages = [...chatState.messages, optimistic];
    chatState.draft = "";
    autoScroll = true;
    await tick();
    autoresize();
    scrollToBottom();
    await api.chatSend(threadId, text, selectedShelfId);
  }

  function stop() {
    if (chatState.pending) {
      api.chatCancel(chatState.pending.messageId);
    }
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

  $effect(() => {
    void chatState.pending?.text;
    void chatState.pending?.thinking;
    void chatState.messages.length;
    if (autoScroll) {
      tick().then(() => autoScroll && scrollToBottom());
    }
  });

  async function removeThread(threadId: string, event: MouseEvent) {
    event.stopPropagation();
    if (!confirm("Delete this conversation? This cannot be undone.")) return;
    await api.threadDelete(threadId);
    if (chatState.activeThreadId === threadId) {
      chatState.activeThreadId = null;
      chatState.messages = [];
    }
    await refreshThreads();
  }

  const suggestions = [
    "Help me write a response to this customer.",
    "Explain this accounting concept.",
    "Give me five ideas for our next campaign.",
    "Draft a follow-up email.",
  ];
</script>

<div class="flex h-full min-h-0">
  <ChatThreadList
    threads={app.threads}
    activeThreadId={chatState.activeThreadId}
    shelves={app.shelves}
    onopen={openThread}
    onnew={newThread}
    onremove={removeThread}
  />

  <section class="flex min-w-0 flex-1 flex-col">
    <div
      bind:this={scrollEl}
      onscroll={onScroll}
      class="min-h-0 flex-1 overflow-y-auto [mask-image:linear-gradient(to_bottom,black_0%,black_calc(100%-25px),transparent_100%)]"
    >
      {#if chatState.messages.length === 0 && !chatState.pending}
        <div class="flex h-full flex-col items-center justify-center px-8">
          <img src={mark} alt="Rebost" class="mb-3 w-[100px] rounded-2xl" />
          <h2 class="text-[19px] font-semibold text-ink">Start a conversation</h2>
          <p class="mt-1 mb-6 max-w-md text-center text-[13px] text-ink-soft">
            Type below. If the answer should come from your files, pick a Shelf first.
          </p>
          <div class="grid w-full max-w-xl grid-cols-2 gap-2">
            {#each suggestions as suggestion}
              <button
                type="button"
                class="card px-3.5 py-3 text-left text-[12.5px] text-ink-soft hover:border-navy-300 hover:text-ink"
                onclick={() => {
                  chatState.draft = suggestion;
                  composerEl?.focus();
                  tick().then(autoresize);
                }}
              >
                {suggestion}
              </button>
            {/each}
          </div>
        </div>
      {:else}
        <div class="mx-auto flex max-w-[760px] flex-col gap-4 px-6 py-6">
          {#each chatState.messages as message (message.id)}
            {#if message.role === "user"}
              <div class="flex justify-end">
                <div
                  class="max-w-[85%] rounded-2xl rounded-br-md bg-navy-900 px-4 py-2.5 text-[13.8px] leading-relaxed text-white select-text cursor-text whitespace-pre-wrap"
                >
                  {message.text}
                </div>
              </div>
            {:else}
              <div class="group flex flex-col gap-1.5">
                <div
                  class="max-w-[92%] rounded-2xl rounded-bl-md border border-paper-line bg-white px-4 py-3 shadow-card"
                >
                  {#if message.thinking}
                    <button
                      type="button"
                      class="mb-1 flex items-center gap-1 text-[11.5px] font-medium text-ink-faint hover:text-ink-soft"
                      onclick={() =>
                        (openThinking = {
                          ...openThinking,
                          [message.id]: !openThinking[message.id],
                        })}
                    >
                      <ChevronDown size={11} class={openThinking[message.id] ? "" : "-rotate-90"} />
                      Thinking
                    </button>
                    {#if openThinking[message.id]}
                      <p
                        class="mb-2.5 border-l-2 border-paper-line pl-3 text-[12px] leading-relaxed whitespace-pre-wrap text-ink-faint select-text cursor-text"
                      >
                        {message.thinking}
                      </p>
                    {/if}
                  {/if}
                  <Markdown
                    text={message.text}
                    sources={message.sources}
                    oncite={(s) => (openSource = s)}
                  />
                  {#if message.sources.length > 0}
                    <div class="mt-2.5 flex flex-wrap gap-1.5 border-t border-paper-line pt-2.5">
                      {#each message.sources as source (source.sid)}
                        <button
                          type="button"
                          class="chip border border-navy-200 bg-navy-50 text-navy-700 hover:bg-amber-350/60 hover:border-amber-450"
                          onclick={() => (openSource = source)}
                        >
                          <span class="font-bold">{source.sid}</span>
                          <span class="max-w-[220px] truncate font-normal">{source.title}</span>
                          {#if source.pageStart}<span class="text-navy-400"
                              >p. {source.pageStart}</span
                            >{/if}
                        </button>
                      {/each}
                    </div>
                  {/if}
                  {#if message.status === "stopped"}
                    <p class="mt-1.5 text-[11px] italic text-ink-faint">Stopped.</p>
                  {/if}
                </div>
                <CopyActions text={message.text} subtle />
              </div>
            {/if}
          {/each}

          {#if chatState.pending}
            <div class="flex flex-col gap-1.5" aria-live="polite" aria-atomic="false">
              <div
                class="max-w-[92%] rounded-2xl rounded-bl-md border border-paper-line bg-white px-4 py-3 shadow-card"
              >
                {#if chatState.pending.phase === "queued"}
                  <p class="flex items-center gap-2 text-[13.5px] text-ink-soft">
                    <span class="inline-block h-2 w-2 animate-pulse rounded-full bg-amber-450"
                    ></span>
                    {warming ? "Warming up…" : "Thinking…"}
                  </p>
                {:else}
                  {#if chatState.pending.thinking}
                    <button
                      type="button"
                      class="mb-1 flex items-center gap-1.5 text-[11.5px] font-medium text-ink-faint hover:text-ink-soft"
                      onclick={() =>
                        (openThinking = { ...openThinking, pending: !openThinking.pending })}
                    >
                      {#if !chatState.pending.text}
                        <span
                          class="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-amber-450"
                        ></span>
                        Thinking…
                      {:else}
                        <ChevronDown size={11} class={openThinking.pending ? "" : "-rotate-90"} />
                        Thinking
                      {/if}
                    </button>
                    {#if openThinking.pending}
                      <p
                        class="mb-2.5 border-l-2 border-paper-line pl-3 text-[12px] leading-relaxed whitespace-pre-wrap text-ink-faint"
                      >
                        {chatState.pending.thinking}
                      </p>
                    {/if}
                  {/if}
                  {#if chatState.pending.text}
                    <Markdown text={chatState.pending.text} streaming />
                  {:else if !chatState.pending.thinking}
                    <p class="flex items-center gap-2 text-[13.5px] text-ink-soft">
                      <span class="inline-block h-2 w-2 animate-pulse rounded-full bg-amber-450"
                      ></span>
                    </p>
                  {/if}
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
      bind:selectedShelfId
      {selectedShelf}
      bind:shelfMenuOpen
      onsend={send}
      onstop={stop}
      onchooseshelf={chooseShelf}
      onautoresize={autoresize}
    />
  </section>
</div>

{#if openSource}
  <SourcePanel source={openSource} onclose={() => (openSource = null)} />
{/if}
