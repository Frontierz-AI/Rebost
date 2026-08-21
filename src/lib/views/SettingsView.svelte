<script lang="ts">
  import {
    api,
    formatBytes,
    type Diagnostics,
    type MachineView,
    type ModelSearchResult,
    type Recommendation,
  } from "$lib/api";
  import { modelBudgetBytes } from "$lib/explore-models";
  import { app, beginModelInstall, notifyInvokeError, refreshSettings } from "$lib/stores.svelte";
  import { HOUSE_RULES_MAX_CHARS, clipChars } from "$lib/text-cap";
  import { Cpu, Search, BadgeCheck, Stethoscope, Save } from "@lucide/svelte";
  import DownloadProgress from "$lib/components/DownloadProgress.svelte";
  import ResetWorkspaceModal from "$lib/components/ResetWorkspaceModal.svelte";
  import ExploreModelsModal from "$lib/components/ExploreModelsModal.svelte";
  import ModelSuggestionCards from "$lib/components/ModelSuggestionCards.svelte";
  import icon from "../../assets/rebost-icon.png";

  let machine = $state<MachineView | null>(null);
  let houseRules = $state(app.settings?.houseRules ?? "");
  let onlineResearch = $state(app.settings?.allowOnlineResearch ?? false);
  let rulesSaved = $state(false);
  let diag = $state<Diagnostics | null>(null);
  let showDiag = $state(false);
  let showReset = $state(false);
  let showExplore = $state(false);
  let resetting = $state(false);
  const hasAi = $derived(!!app.settings?.activeModel);

  $effect(() => {
    void app.settings?.activeModel?.reference;
    api
      .machineProfile()
      .then((m) => (machine = m))
      .catch(notifyInvokeError);
  });

  $effect(() => {
    houseRules = clipChars(app.settings?.houseRules ?? "", HOUSE_RULES_MAX_CHARS);
    onlineResearch = app.settings?.allowOnlineResearch ?? false;
  });

  const modelDownload = $derived(
    Object.values(app.downloads).find((d) => d.kind === "model" && !d.done && !d.error),
  );
  const engineDownload = $derived(
    Object.values(app.downloads).find((d) => d.kind === "engine" && !d.done && !d.error),
  );

  function installFrom(result: ModelSearchResult) {
    void install(
      result.source.includes("huggingface") ? "huggingface" : "ollama",
      result.reference,
      result.name,
      result.license,
      result.sizeBytes,
    );
  }

  async function install(
    source: string,
    reference: string,
    name: string,
    license?: string,
    total?: number | null,
  ) {
    await beginModelInstall(source, reference, name, license, total);
  }

  function installRec(rec: Recommendation) {
    void install("huggingface", rec.reference, rec.name, rec.license, rec.approxBytes);
  }

  async function saveRules() {
    await api.setHouseRules(clipChars(houseRules, HOUSE_RULES_MAX_CHARS));
    await refreshSettings();
    rulesSaved = true;
    setTimeout(() => (rulesSaved = false), 1500);
  }

  async function saveOnline() {
    const next = onlineResearch;
    try {
      await api.setAllowOnlineResearch(next);
      await refreshSettings();
    } catch (error) {
      onlineResearch = app.settings?.allowOnlineResearch ?? false;
      notifyInvokeError(error);
    }
  }

  async function openDiagnostics() {
    showDiag = !showDiag;
    if (showDiag) diag = await api.diagnostics();
  }

  async function resetWorkspace() {
    resetting = true;
    try {
      await api.resetWorkspace("DELETE");
    } catch (error) {
      resetting = false;
      notifyInvokeError(error);
    }
  }
</script>

<div class="mx-auto max-w-[760px] px-8 py-8">
  <h1 class="mb-6 text-[22px] font-semibold text-ink">Settings</h1>

  {#if hasAi}
    {@render houseRulesSection()}
    {@render onlineSection()}
    {@render aiSection()}
  {:else}
    {@render aiSection()}
    {@render houseRulesSection()}
    {@render onlineSection()}
  {/if}

  {#snippet houseRulesSection()}
    <section class="card mb-6 px-6 py-5">
      <h2 class="mb-1 text-[15px] font-semibold text-ink">House rules</h2>
      <p class="mb-3 text-[12.5px] leading-snug text-ink-soft">
        Standing instructions: tone, language, what never to promise. Rebost follows them in every
        conversation and Recipe.
      </p>
      <label class="sr-only" for="house-rules">House rules</label>
      <textarea
        id="house-rules"
        name="house-rules"
        class="input min-h-32 cursor-text resize-y select-text"
        placeholder={"Examples:\n· Answer in the language of the documents unless asked otherwise.\n· We are a furniture workshop. Use first names.\n· Never promise delivery dates; say the team will confirm."}
        maxlength={HOUSE_RULES_MAX_CHARS}
        bind:value={houseRules}></textarea>
      <div class="mt-2.5 flex justify-end">
        <button type="button" class="btn-primary" onclick={saveRules}>
          <Save size={13.5} />
          {rulesSaved ? "Saved" : "Save house rules"}
        </button>
      </div>
    </section>
  {/snippet}

  {#snippet onlineSection()}
    <section class="card mb-6 px-6 py-5">
      <label class="flex cursor-default items-start gap-3" for="online-research">
        <input
          id="online-research"
          name="online-research"
          type="checkbox"
          aria-describedby="online-research-help"
          class="mt-1.5 size-4 shrink-0 rounded border-paper-line accent-navy-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-800"
          bind:checked={onlineResearch}
          onchange={() => void saveOnline()}
        />
        <span>
          <h2 class="text-[15px] font-semibold text-ink">
            Allow your AI to conduct online research
          </h2>
          <p id="online-research-help" class="mt-1 text-[12.5px] leading-snug text-ink-soft">
            Turn this on so your AI can search the public web. Your files are not sent online. Once
            the pages are in, the answer is still written on this computer.
          </p>
        </span>
      </label>
    </section>
  {/snippet}

  {#snippet aiSection()}
    <section class="card mb-6 px-6 py-5">
      <h2 class="mb-1 text-[15px] font-semibold text-ink">AI Brain</h2>
      {#if machine}
        <p class="mb-4 flex items-center gap-1.5 text-[12px] text-ink-faint">
          <Cpu size={12.5} />
          {machine.profile.cpu} · {formatBytes(machine.profile.totalRamBytes)} memory ·
          {formatBytes(machine.profile.freeDiskBytes)} free disk
        </p>
      {/if}

      {#if modelDownload}
        <div class="mb-4 rounded-xl border border-paper-line bg-paper-soft px-4 py-3">
          <DownloadProgress
            download={modelDownload}
            cancelable
            note={app.settings?.activeModel
              ? "You can keep using Chat while this finishes."
              : undefined}
          />
        </div>
      {:else if engineDownload}
        <div class="mb-4 rounded-xl border border-paper-line bg-paper-soft px-4 py-3">
          <DownloadProgress download={engineDownload} cancelable />
        </div>
      {/if}

      {#if app.settings?.activeModel}
        {@const model = app.settings.activeModel}
        <div
          class="flex items-center gap-3 rounded-xl border border-paper-line bg-paper-soft/50 px-4 py-3"
        >
          <BadgeCheck size={18} class="shrink-0 text-navy-600 dark:text-navy-400" />
          <div class="min-w-0 flex-1">
            <p class="text-[13.5px] font-semibold text-ink">{model.name}</p>
            <p class="text-[11.5px] text-ink-soft">
              {formatBytes(model.sizeBytes)}{model.license ? ` · ${model.license}` : ""} · installed on
              this computer
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
            {app.engine.state === "ready"
              ? "Ready"
              : app.engine.state === "starting"
                ? "Warming up…"
                : "Idle"}
          </span>
        </div>
      {/if}

      {#if machine && machine.suggestions.length > 0}
        <div class="mt-5">
          <ModelSuggestionCards
            suggestions={machine.suggestions}
            installing={!!modelDownload}
            heading={app.settings?.activeModel
              ? "Other AIs that fit"
              : "Suggested for this computer"}
            lede={app.settings?.activeModel
              ? "Other picks sized for this computer."
              : "Sized for this computer."}
            onInstall={installRec}
          />
        </div>
      {/if}

      <div
        class="mt-5 flex items-center gap-4 border-t border-navy-950/10 pt-5 dark:border-white/10"
      >
        <div class="min-w-0 flex-1">
          <h3 class="text-[15px] font-semibold text-ink">Explore other AIs</h3>
          <p class="mt-0.5 text-[12.5px] leading-snug text-ink-soft">
            Browse public catalogs for something else. Only the search words leave this computer.
          </p>
        </div>
        <button
          type="button"
          class="btn-outline shrink-0"
          aria-haspopup="dialog"
          aria-expanded={showExplore}
          aria-controls={showExplore ? "explore-models-dialog" : undefined}
          onclick={() => (showExplore = true)}
        >
          <Search size={13.5} aria-hidden="true" />
          Browse AIs
        </button>
      </div>
    </section>
  {/snippet}

  <section class="card mb-6 px-6 py-5">
    <div class="flex items-center gap-4">
      <img src={icon} alt="" class="h-12 w-12 rounded-[22%]" />
      <div class="min-w-0 flex-1">
        <h2 class="text-[15px] font-semibold text-ink">About Rebost</h2>
        <p class="mt-0.5 text-[12.5px] leading-snug text-ink-soft">
          Private AI that works with your files.
        </p>
      </div>
      <button
        type="button"
        class="btn-outline shrink-0"
        onclick={() => api.showAboutWindow().catch(notifyInvokeError)}
      >
        Open
      </button>
    </div>
  </section>

  <section class="card mb-6 px-6 py-5">
    <div class="flex items-center gap-4">
      <div class="min-w-0 flex-1">
        <h2 class="text-[15px] font-semibold text-ink">Reset Rebost</h2>
        <p class="mt-0.5 text-[12.5px] leading-snug text-ink-soft">
          Deletes conversations, settings, House rules, Recipes, what Rebost has read, and the AI.
          Your own files stay on disk. Rebost forgets the Shelves that pointed at them.
        </p>
      </div>
      <button
        type="button"
        class="btn-outline shrink-0 text-red-700 hover:border-red-300 hover:bg-red-50 dark:text-red-400 dark:hover:border-red-400/40 dark:hover:bg-red-400/10"
        aria-haspopup="dialog"
        aria-expanded={showReset}
        aria-controls={showReset ? "reset-workspace-dialog" : undefined}
        onclick={() => (showReset = true)}
      >
        Reset
      </button>
    </div>
  </section>

  <section class="mb-10">
    <button type="button" class="btn-ghost !text-[12px]" onclick={openDiagnostics}>
      <Stethoscope size={13} />
      {showDiag ? "Hide diagnostics" : "Diagnostics"}
    </button>
    {#if showDiag && diag}
      <div class="card mt-2 px-5 py-4 font-mono text-[11.5px] leading-relaxed text-ink-soft">
        <p>
          Rebost {diag.version} · engine {diag.engineBuild} ({diag.engineState.state}{diag
            .engineState.detail
            ? ` · ${diag.engineState.detail}`
            : ""})
        </p>
        <p>data: {diag.dataDir}</p>
        <p class="mt-1 text-[10.5px] text-ink-faint">
          The data folder path is local to this computer. Do not paste it into a public issue.
        </p>
        <p>model: {diag.model ? `${diag.model.name} · ${diag.model.file}` : "none installed"}</p>
        <p>index records: {diag.indexRecords} · context budget: {diag.contextBudgetChars} chars</p>
        {#if diag.benchmark}
          <p>
            benchmark: {diag.benchmark.promptTokensPerSecond.toFixed(0)} pp tok/s · {diag.benchmark.generationTokensPerSecond.toFixed(
              0,
            )} gen tok/s · {diag.benchmark.measuredAt}
          </p>
        {/if}
        <p>
          machine: {diag.machine.cpu} · {formatBytes(diag.machine.totalRamBytes)} RAM · {diag
            .machine.accelerator}
          {diag.machine.processArch !== diag.machine.osArch
            ? ` · ${diag.machine.processArch} on ${diag.machine.osArch}`
            : ` · ${diag.machine.processArch}`}
        </p>
        <p>formats: {diag.supportedFormats.join(" ")}</p>
        {#if diag.engineLogPresent}
          <p class="mt-2">
            engine log location:
            <button
              type="button"
              class="rounded-sm text-left break-all underline decoration-navy-200 underline-offset-2 hover:text-ink hover:decoration-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-800"
              aria-label="Open engine log"
              onclick={() => api.openEngineLog().catch(notifyInvokeError)}
            >
              <code>{diag.engineLogPath}</code>
            </button>
          </p>
          <p class="mt-1 text-[10.5px] leading-relaxed text-ink-faint">
            Log contents stay on disk (they can name local files).
          </p>
        {/if}
      </div>
    {/if}
  </section>
</div>

{#if showExplore}
  <ExploreModelsModal
    installing={!!modelDownload}
    budgetBytes={modelBudgetBytes(machine?.profile.totalRamBytes)}
    onClose={() => (showExplore = false)}
    onInstall={(result) => {
      installFrom(result);
      showExplore = false;
    }}
  />
{/if}
{#if showReset}
  <ResetWorkspaceModal
    busy={resetting}
    onClose={() => {
      if (!resetting) showReset = false;
    }}
    onConfirm={resetWorkspace}
  />
{/if}
