<script lang="ts">
  import { tick } from "svelte";
  import { fade } from "svelte/transition";
  import { Lock, LibraryBig, ChefHat, Download, Check, ChevronDown } from "@lucide/svelte";
  import {
    api,
    downloadErrorMessage,
    formatBytes,
    type MachineView,
    type Recommendation,
  } from "$lib/api";
  import DownloadProgress from "$lib/components/DownloadProgress.svelte";
  import ModelCatalogInfo from "$lib/components/ModelCatalogInfo.svelte";
  import { accordion, installCard, motionMs } from "$lib/motion";
  import { app, beginModelInstall, notifyInvokeError, refreshSettings } from "$lib/stores.svelte";
  import mark from "../../assets/R.webp";

  let step = $state<"promise" | "model">("promise");
  let machine = $state<MachineView | null>(null);
  let installing = $state(false);
  let showMore = $state(false);
  let lastRec = $state<Recommendation | null>(null);
  let installHeading = $state<HTMLHeadingElement | null>(null);

  $effect(() => {
    api
      .machineProfile()
      .then((m) => (machine = m))
      .catch(notifyInvokeError);
  });

  const download = $derived(
    Object.values(app.downloads).find(
      (d) => (d.kind === "model" || d.kind === "engine") && !d.done && !d.error,
    ),
  );
  const modelDone = $derived(!!app.settings?.activeModel);
  const busy = $derived(!!download || installing);
  const failedDownload = $derived.by(() => {
    if (download || modelDone) return undefined;
    const preferredId = lastRec ? `model:${lastRec.reference}` : null;
    if (preferredId) {
      const preferred = app.downloads[preferredId];
      if (preferred?.error && preferred.error !== "cancelled") return preferred;
    }
    return Object.values(app.downloads).find(
      (d) => (d.kind === "model" || d.kind === "engine") && !!d.error && d.error !== "cancelled",
    );
  });
  const failedMessage = $derived(
    failedDownload?.error ? downloadErrorMessage(failedDownload.error) : null,
  );

  async function installRec(rec: Recommendation) {
    lastRec = rec;
    installing = true;
    try {
      await beginModelInstall("huggingface", rec.reference, rec.name, rec.license, rec.approxBytes);
    } catch (error) {
      installing = false;
      notifyInvokeError(error);
    }
  }

  async function skip() {
    const inFlight = Object.values(app.downloads).filter(
      (d) => (d.kind === "model" || d.kind === "engine") && !d.done && !d.error,
    );
    await Promise.all(inFlight.map((d) => api.downloadCancel(d.id).catch(() => undefined)));
    await finish();
  }

  async function finish() {
    try {
      await api.finishOnboarding();
      await refreshSettings();
      app.onboarding = false;
    } catch (error) {
      notifyInvokeError(error);
    }
  }

  $effect(() => {
    if (installing && Object.values(app.downloads).some((d) => d.kind === "model" && d.error)) {
      installing = false;
    }
  });

  $effect(() => {
    if (installing && modelDone && !download) {
      finish();
    }
  });

  $effect(() => {
    if (step !== "model") return;
    void tick().then(() => installHeading?.focus());
  });
</script>

<div
  data-tauri-drag-region
  class="flex h-full items-center justify-center overflow-y-auto bg-navy-950 p-8 select-none"
>
  <div data-tauri-drag-region="false" class="onboard-stage w-[600px] py-4 select-none">
    {#if step === "promise"}
      <div
        class="onboard-pane flex flex-col items-center text-center"
        out:fade={{ duration: motionMs(200) }}
      >
        <img src={mark} alt="Rebost" class="mb-5 w-[100px] rounded-2xl" />
        <h1 class="text-[28px] font-bold text-white">Welcome to Rebost</h1>
        <p class="mt-2 max-w-md text-[15px] leading-relaxed text-navy-200">
          Private AI that works with your files. What happens in your computer stays in your
          computer.
        </p>
        <div class="mt-8 grid w-full grid-cols-3 gap-3">
          <div class="onboard-card rounded-xl bg-white/6 px-4 py-4 text-left">
            <Lock size={17} class="mb-2 text-amber-450" />
            <p class="text-[12.5px] font-semibold text-white">On this computer</p>
            <p class="mt-1 min-h-[4.5rem] text-[11.5px] leading-snug text-navy-200">
              Chats and files stay here. After you install an AI, answers are generated on this
              computer.
            </p>
          </div>
          <div
            class="onboard-card rounded-xl bg-white/6 px-4 py-4 text-left"
            style="animation-delay: 60ms"
          >
            <LibraryBig size={17} class="mb-2 text-amber-450" />
            <p class="text-[12.5px] font-semibold text-white">Private shelves and files</p>
            <p class="mt-1 min-h-[4.5rem] text-[11.5px] leading-snug text-navy-200">
              Add files to a Shelf and your AI will cite the pages where it found the answers.
            </p>
          </div>
          <div
            class="onboard-card rounded-xl bg-white/6 px-4 py-4 text-left"
            style="animation-delay: 120ms"
          >
            <ChefHat size={17} class="mb-2 text-amber-450" />
            <p class="text-[12.5px] font-semibold text-white">Your best recipes</p>
            <p class="mt-1 min-h-[4.5rem] text-[11.5px] leading-snug text-navy-200">
              Save instructions and ideas you use often, so they're just one click away next time.
            </p>
          </div>
        </div>
        <button
          type="button"
          class="btn-amber mt-9 !px-7 !py-2.5 !text-[14px]"
          onclick={() => (step = "model")}
        >
          Continue
        </button>
      </div>
    {:else}
      <section
        class="onboard-pane flex flex-col gap-8 rounded-2xl bg-surface p-8 shadow-pop dark:shadow-none dark:inset-ring dark:inset-ring-white/5"
        aria-labelledby="onboard-install-heading"
        aria-busy={busy}
        in:installCard
      >
        {#snippet skipButton()}
          <button
            type="button"
            class="btn-ghost relative ml-auto shrink-0 !text-[0.8125rem]"
            onclick={skip}
          >
            <span
              class="absolute top-1/2 left-1/2 size-[max(100%,3rem)] -translate-1/2 pointer-fine:hidden"
              aria-hidden="true"
            ></span>
            Skip and download it later
          </button>
        {/snippet}
        <div class="flex flex-col gap-2">
          <p class="text-[0.8125rem] font-medium text-ink-soft">Install an AI</p>
          <h1
            id="onboard-install-heading"
            bind:this={installHeading}
            tabindex="-1"
            class="max-w-[40ch] text-[1.5rem] font-semibold tracking-tight text-balance text-ink outline-none"
          >
            Rebost needs to download a brain first.
          </h1>
          <p class="max-w-[40ch] text-[1rem] text-pretty text-ink">
            That's the AI that answers you on this computer. It can take a few minutes.
          </p>
        </div>

        {#if download}
          <div class="flex flex-col gap-3">
            <div class="rounded-xl bg-paper-soft p-5 dark:inset-ring dark:inset-ring-white/5">
              <DownloadProgress
                {download}
                cancelable
                note={download.phase === "verifying"
                  ? "Making sure the file arrived intact."
                  : "Rebost will be ready when it finishes."}
              />
            </div>
            <div class="flex justify-end">
              {@render skipButton()}
            </div>
          </div>
        {:else if modelDone}
          <div class="flex flex-col gap-4">
            <p
              class="flex items-baseline gap-2 text-[0.875rem] font-medium text-emerald-700 dark:text-emerald-400"
            >
              <Check size={16} class="size-4 h-lh shrink-0" aria-hidden="true" />
              The AI is installed.
            </p>
            <button
              type="button"
              class="btn-primary self-start focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-400"
              onclick={finish}
            >
              Start using Rebost
            </button>
          </div>
        {:else}
          <div class="flex flex-col gap-3">
            {#if machine}
              <p class="max-w-[48ch] text-[0.875rem] text-pretty text-ink-soft">
                There are several free AIs. Rebost picked one that should run well here.
              </p>
              <div
                class="flex items-start gap-4 rounded-xl bg-paper-soft p-5 dark:inset-ring dark:inset-ring-white/5"
              >
                <div class="flex min-w-0 flex-1 flex-col gap-1">
                  <p class="text-[0.8125rem] font-medium text-ink-soft">
                    {failedDownload ? "Couldn't install" : "Chosen for this computer"}
                  </p>
                  <p
                    class="flex min-w-0 items-center gap-1 text-[1.0625rem] font-semibold text-ink"
                  >
                    <span class="min-w-0 truncate">{(lastRec ?? machine.recommendation).name}</span>
                    <ModelCatalogInfo rec={lastRec ?? machine.recommendation} />
                  </p>
                  <p class="text-[0.8125rem] text-pretty text-ink-soft tabular-nums">
                    About {formatBytes((lastRec ?? machine.recommendation).approxBytes)} to download.
                  </p>
                  {#if failedMessage}
                    <p
                      class="mt-1 text-[0.8125rem] text-pretty text-red-700 dark:text-red-400"
                      role="alert"
                    >
                      {failedMessage}
                    </p>
                  {/if}
                </div>
                {#if failedDownload}
                  <button
                    type="button"
                    class="btn-amber shrink-0 py-2 pr-3 pl-2 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-400"
                    onclick={() => machine && installRec(lastRec ?? machine.recommendation)}
                  >
                    Try again
                  </button>
                {:else if !busy}
                  <button
                    type="button"
                    class="btn-amber shrink-0 py-2 pr-3 pl-2 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-400"
                    onclick={() => machine && installRec(machine.recommendation)}
                  >
                    <Download size={16} class="size-4 h-lh shrink-0" aria-hidden="true" />
                    Install
                  </button>
                {/if}
              </div>

              <div class="flex items-center gap-3">
                {#if machine.alternatives.length > 0}
                  <button
                    type="button"
                    class="btn-ghost relative py-2 pr-2 pl-3 !text-[0.8125rem]"
                    onclick={() => (showMore = !showMore)}
                    aria-expanded={showMore}
                    aria-controls="onboard-other-ais"
                  >
                    <span
                      class="absolute top-1/2 left-1/2 size-[max(100%,3rem)] -translate-1/2 pointer-fine:hidden"
                      aria-hidden="true"
                    ></span>
                    Choose a different AI
                    <ChevronDown
                      size={16}
                      class="size-4 h-lh shrink-0 {showMore ? 'rotate-180' : ''}"
                      aria-hidden="true"
                    />
                  </button>
                {/if}
                {@render skipButton()}
              </div>
              {#if showMore && machine.alternatives.length > 0}
                <ul
                  id="onboard-other-ais"
                  class="overflow-hidden rounded-xl ring-1 ring-navy-950/10 dark:ring-white/10"
                  role="list"
                  transition:accordion
                >
                  {#each machine.alternatives as alt (alt.reference)}
                    <li
                      class="flex items-start gap-3 px-4 py-3 not-last:border-b not-last:border-paper-line"
                    >
                      <div class="min-w-0 flex-1">
                        <p
                          class="flex min-w-0 items-center gap-1 text-[0.875rem] font-semibold text-ink"
                        >
                          <span class="min-w-0 truncate">{alt.name}</span>
                          <ModelCatalogInfo rec={alt} />
                        </p>
                        <p class="text-[0.8125rem] text-ink-soft tabular-nums">
                          About {formatBytes(alt.approxBytes)} to download.
                        </p>
                      </div>
                      <button
                        type="button"
                        class="btn-outline shrink-0 py-1.5 pr-2.5 pl-2.5 !text-[0.75rem]"
                        onclick={() => installRec(alt)}
                        disabled={busy}
                      >
                        Install
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            {:else}
              <div class="flex justify-end">
                {@render skipButton()}
              </div>
            {/if}
          </div>
        {/if}
      </section>
    {/if}
  </div>
</div>

<style>
  .onboard-stage {
    display: grid;
    align-items: center;
    justify-items: stretch;
  }
  .onboard-pane {
    grid-area: 1 / 1;
  }
  @media (prefers-reduced-motion: no-preference) {
    .onboard-card {
      animation: onboard-card-in 280ms cubic-bezier(0.32, 0.72, 0, 1) both;
    }
  }
  @keyframes onboard-card-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
