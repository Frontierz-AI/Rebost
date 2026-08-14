<script lang="ts">
  import {
    api,
    formatBytes,
    formatReleased,
    type MachineView,
    type Recommendation,
  } from "$lib/api";
  import { app, beginModelInstall, refreshSettings } from "$lib/stores.svelte";
  import { Lock, LibraryBig, ChefHat, Download, Check } from "@lucide/svelte";
  import DownloadProgress from "$lib/components/DownloadProgress.svelte";
  import mark from "../../assets/R.webp";

  let step = $state<"promise" | "model">("promise");
  let machine = $state<MachineView | null>(null);
  let installing = $state(false);
  let showMore = $state(false);

  $effect(() => {
    api.machineProfile().then((m) => (machine = m));
  });

  const download = $derived(
    Object.values(app.downloads).find(
      (d) => (d.kind === "model" || d.kind === "engine") && !d.done && !d.error,
    ),
  );
  const modelDone = $derived(!!app.settings?.activeModel);
  const busy = $derived(!!download || installing);

  async function installRec(rec: Recommendation) {
    installing = true;
    await beginModelInstall("huggingface", rec.reference, rec.name, rec.license, rec.approxBytes);
  }

  async function finish() {
    await api.finishOnboarding();
    await refreshSettings();
    app.onboarding = false;
    api.warmEngine().catch(() => {});
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
</script>

{#snippet facts(rec: Recommendation, featured: boolean)}
  <dl class="mt-3 grid grid-cols-3 gap-x-4 {featured ? 'text-[0.75rem]' : 'text-[0.6875rem]'}">
    <div>
      <dt class="text-ink-faint">From</dt>
      <dd class="text-ink">{rec.provider}</dd>
    </div>
    <div>
      <dt class="text-ink-faint">Download</dt>
      <dd class="text-ink">About {formatBytes(rec.approxBytes)}</dd>
    </div>
    <div>
      <dt class="text-ink-faint">Released</dt>
      <dd class="text-ink">{formatReleased(rec.released)}</dd>
    </div>
  </dl>
{/snippet}

<div
  data-tauri-drag-region
  class="flex h-full select-none items-center justify-center overflow-y-auto bg-navy-950 p-8"
>
  <div data-tauri-drag-region="false" class="w-[600px] select-none py-4">
    {#if step === "promise"}
      <div class="flex flex-col items-center text-center">
        <img src={mark} alt="Rebost" class="mb-5 w-[100px] rounded-2xl" />
        <h1 class="text-[28px] font-bold text-white">Welcome to Rebost</h1>
        <p class="mt-2 max-w-md text-[15px] leading-relaxed text-navy-200">
          Private AI that lives with your files and never leaves them.
        </p>
        <div class="mt-8 grid w-full grid-cols-3 gap-3">
          <div class="rounded-xl bg-white/6 px-4 py-4 text-left">
            <Lock size={17} class="mb-2 text-amber-450" />
            <p class="text-[12.5px] font-semibold text-white">On this computer</p>
            <p class="mt-1 min-h-[4.5rem] text-[11.5px] leading-snug text-navy-200">
              Chats and files stay here. After you install a model, Rebost works fully 
              offline.
            </p>
          </div>
          <div class="rounded-xl bg-white/6 px-4 py-4 text-left">
            <LibraryBig size={17} class="mb-2 text-amber-450" />
            <p class="text-[12.5px] font-semibold text-white">Private shelves and files</p>
            <p class="mt-1 min-h-[4.5rem] text-[11.5px] leading-snug text-navy-200">
              Add files to a Shelf and your AI will cite the pages where it found the
              answers.
            </p>
          </div>
          <div class="rounded-xl bg-white/6 px-4 py-4 text-left">
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
      <div class="flex flex-col gap-5 rounded-2xl bg-white p-7 shadow-pop">
        <div>
          <h2 class="text-[1.1875rem] font-semibold text-ink">Install a model</h2>
          <p class="mt-1 whitespace-nowrap text-[0.8125rem] text-ink-soft">
            Picked for the RAM on this computer.
          </p>
        </div>

        {#if machine}
          <div class="rounded-xl border border-amber-450/60 bg-amber-350/15 px-5 py-4">
            <p class="text-[0.6875rem] font-bold tracking-wide text-amber-550 uppercase">
              Fits this computer
            </p>
            <p class="mt-1 text-[1.0625rem] font-semibold text-ink">
              {machine.recommendation.name}
            </p>
            <p class="mt-1 text-[0.8125rem] text-ink-soft">
              Largest catalog model that should run without swapping.
            </p>
            {@render facts(machine.recommendation, true)}
            {#if !modelDone && !busy}
              <button
                type="button"
                class="btn-amber mt-3"
                onclick={() => machine && installRec(machine.recommendation)}
              >
                <Download size={14} /> Install
              </button>
            {/if}
          </div>
        {/if}

        {#if download}
          <DownloadProgress
            {download}
            note={download.phase === "verifying"
              ? "Making sure the file arrived intact."
              : "You can leave this window in the background. Rebost will be ready when it finishes."}
          />
        {:else if modelDone}
          <p class="flex items-center gap-2 text-[0.84375rem] font-medium text-emerald-700">
            <Check size={15} /> The model is installed.
          </p>
        {/if}

        {#if !busy && showMore && machine && machine.alternatives.length > 0}
          <div class="flex flex-col gap-2">
            <p class="text-[0.6875rem] font-bold tracking-wide text-ink-faint uppercase">
              Other good picks
            </p>
            <div class="overflow-hidden rounded-xl border border-navy-950/10">
              {#each machine.alternatives as alt (alt.reference)}
                <div class="flex items-start gap-3 border-navy-950/10 px-4 py-3 not-last:border-b">
                  <div class="min-w-0 flex-1">
                    <p class="text-[0.875rem] font-semibold text-ink">{alt.name}</p>
                    <p class="mt-0.5 text-[0.75rem] text-ink-soft">{alt.blurb}</p>
                    {@render facts(alt, false)}
                  </div>
                  {#if !modelDone && !download}
                    <button
                      type="button"
                      class="btn-outline mt-0.5 shrink-0 !py-1.5 !text-[0.75rem]"
                      onclick={() => installRec(alt)}
                      disabled={busy}
                    >
                      Install
                    </button>
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if modelDone && !download}
          <div class="flex items-center justify-end">
            <button type="button" class="btn-primary" onclick={finish}>Start using Rebost</button>
          </div>
        {:else if !busy}
          <div class="flex items-center justify-between">
            <button type="button" class="btn-ghost !text-[0.75rem]" onclick={finish}>
              Not now. I'll install it later in Settings
            </button>
            {#if !showMore && machine && machine.alternatives.length > 0}
              <button
                type="button"
                class="btn-ghost !text-[0.8125rem]"
                onclick={() => (showMore = true)}
              >
                Show me more models
              </button>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>
