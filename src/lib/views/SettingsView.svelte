<script lang="ts">
  import {
    api,
    formatBytes,
    formatCount,
    type Diagnostics,
    type MachineView,
    type ModelSearchResult,
    type Recommendation,
  } from "$lib/api";
  import { app, beginModelInstall, notifyInvokeError, refreshSettings } from "$lib/stores.svelte";
  import { HOUSE_RULES_MAX_CHARS, clipChars } from "$lib/text-cap";
  import { Cpu, Search, Download, BadgeCheck, Stethoscope, Save, Info } from "@lucide/svelte";
  import DownloadProgress from "$lib/components/DownloadProgress.svelte";
  import ResetWorkspaceModal from "$lib/components/ResetWorkspaceModal.svelte";
  import ModelInfoModal from "$lib/components/ModelInfoModal.svelte";
  import ModelSuggestionCards from "$lib/components/ModelSuggestionCards.svelte";
  import icon from "../../assets/rebost-icon.png";

  let machine = $state<MachineView | null>(null);
  let searchQuery = $state("");
  let searching = $state(false);
  let results = $state<ModelSearchResult[] | null>(null);
  // Not $state: bumping it in the clear-query effect would loop forever.
  let searchGen = 0;
  let info = $state<ModelSearchResult | null>(null);
  let houseRules = $state(app.settings?.houseRules ?? "");
  let onlineResearch = $state(app.settings?.allowOnlineResearch ?? false);
  let rulesSaved = $state(false);
  let diag = $state<Diagnostics | null>(null);
  let showDiag = $state(false);
  let showReset = $state(false);
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

  $effect(() => {
    if (searchQuery.trim() !== "") return;
    searchGen += 1;
    results = null;
    searching = false;
    info = null;
  });

  /** Tab-focus a name only when it actually overflows and can scroll. */
  function overflowX(node: HTMLElement, label: string) {
    const sync = () => {
      const overflow = node.scrollWidth > node.clientWidth + 1;
      if (overflow) {
        node.tabIndex = 0;
        node.setAttribute("aria-label", label);
      } else {
        node.removeAttribute("tabindex");
        node.removeAttribute("aria-label");
      }
    };
    const observer = new ResizeObserver(sync);
    observer.observe(node);
    requestAnimationFrame(sync);
    return {
      update(next: string) {
        label = next;
        sync();
      },
      destroy() {
        observer.disconnect();
      },
    };
  }

  async function searchModels() {
    const query = searchQuery.trim();
    if (!query) {
      searchGen += 1;
      results = null;
      searching = false;
      info = null;
      return;
    }
    const gen = ++searchGen;
    searching = true;
    try {
      const found = await api.modelsSearch(query);
      if (gen !== searchGen) return;
      results = found;
    } catch (error) {
      if (gen !== searchGen) return;
      notifyInvokeError(error);
      results = [];
    } finally {
      if (gen === searchGen) searching = false;
    }
  }

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
          class="mt-1.5 size-4 shrink-0 rounded border-paper-line accent-navy-800 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-400"
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
        <div class="mb-4 rounded-xl border border-navy-200 bg-navy-50/60 px-4 py-3">
          <DownloadProgress
            download={modelDownload}
            cancelable
            note={app.settings?.activeModel
              ? "You can keep using Chat while this finishes."
              : undefined}
          />
        </div>
      {:else if engineDownload}
        <div class="mb-4 rounded-xl border border-navy-200 bg-navy-50/60 px-4 py-3">
          <DownloadProgress download={engineDownload} cancelable />
        </div>
      {/if}

      {#if app.settings?.activeModel}
        {@const model = app.settings.activeModel}
        <div
          class="flex items-center gap-3 rounded-xl border border-paper-line bg-paper-soft/50 px-4 py-3"
        >
          <BadgeCheck size={18} class="shrink-0 text-emerald-600" />
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
              ? 'bg-emerald-100 text-emerald-800'
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

      <div class="mt-5">
        <h3 class="label mb-2">Explore other AIs</h3>
        <div class="flex gap-2">
          <div class="relative flex-1">
            <Search size={13.5} class="absolute top-1/2 left-3 -translate-y-1/2 text-ink-faint" />
            <label class="sr-only" for="model-search">Search for an AI</label>
            <input
              id="model-search"
              name="model-search"
              class="input !pl-9"
              placeholder="Search: Muse, Gemma, Mistral…"
              bind:value={searchQuery}
              onkeydown={(e) => e.key === "Enter" && searchModels()}
            />
          </div>
          <button type="button" class="btn-outline" onclick={searchModels} disabled={searching}>
            {searching ? "Searching…" : "Search"}
          </button>
        </div>
        <p class="mt-1.5 text-[11px] text-ink-faint">
          Original publishers first, then newest. Download counts come from Hugging Face. Only the
          search words leave this computer.
        </p>

        {#if results !== null}
          <div class="mt-3 overflow-hidden rounded-xl border border-paper-line">
            {#each results as result (result.source + result.reference)}
              <div
                class="flex items-center gap-3 border-b border-paper-line/70 bg-surface px-4 py-2.5 last:border-b-0"
              >
                <div class="min-w-0 flex-1">
                  <p class="flex min-w-0 items-center gap-2 text-[13px] font-medium text-ink">
                    <span
                      class="min-w-0 flex-1 overflow-x-auto overflow-y-hidden overscroll-x-contain whitespace-nowrap"
                      use:overflowX={result.name}>{result.name}</span
                    >
                    {#if result.official}
                      <span
                        class="shrink-0 rounded-full bg-navy-100 px-2 py-0.5 text-[10px] font-semibold text-navy-800"
                      >
                        Official
                      </span>
                    {/if}
                    {#if result.fits === false}
                      <span
                        class="shrink-0 rounded-full bg-paper-soft px-2 py-0.5 text-[10px] font-semibold text-ink-faint"
                      >
                        Too large for this computer
                      </span>
                    {:else if result.fits}
                      <span
                        class="shrink-0 rounded-full bg-emerald-100 px-2 py-0.5 text-[10px] font-semibold text-emerald-800"
                      >
                        Fits this computer
                      </span>
                    {/if}
                  </p>
                  <p class="truncate text-[11.5px] text-ink-faint">
                    {[
                      result.publisher,
                      result.downloads != null && result.downloads > 0
                        ? `${formatCount(result.downloads)} downloads`
                        : null,
                      result.sizeBytes != null ? formatBytes(result.sizeBytes) : null,
                      result.license,
                      result.released,
                      result.source.replace("+", " + "),
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </p>
                </div>
                <div class="flex shrink-0 items-center gap-1.5">
                  <button
                    type="button"
                    class="btn-outline shrink-0 !py-1.5 !text-[12px]"
                    aria-haspopup="dialog"
                    aria-expanded={info?.reference === result.reference &&
                      info?.source === result.source}
                    aria-controls={info?.reference === result.reference
                      ? "model-info-dialog"
                      : undefined}
                    onclick={() => (info = result)}
                  >
                    <Info size={12.5} aria-hidden="true" /> More info
                  </button>
                  <button
                    type="button"
                    class="btn-outline shrink-0 !py-1.5 !text-[12px]"
                    onclick={() => installFrom(result)}
                    disabled={!!modelDownload}
                  >
                    <Download size={12.5} aria-hidden="true" /> Install
                  </button>
                </div>
              </div>
            {:else}
              <p class="bg-surface px-4 py-3 text-[12.5px] text-ink-soft">
                Nothing found for that search.
              </p>
            {/each}
          </div>
          {#if results.length > 0}
            <p class="mt-1.5 text-[11px] text-ink-faint">
              Rebost uses one AI at a time, sized for this computer.
            </p>
          {/if}
        {/if}
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
        class="btn-outline shrink-0 text-red-700 hover:border-red-300 hover:bg-red-50"
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
        </p>
        <p>formats: {diag.supportedFormats.join(" ")}</p>
        {#if diag.engineLogPresent}
          <details class="mt-2">
            <summary>engine log location</summary>
            <p class="mt-1 text-[10.5px] leading-relaxed">
              Log contents stay on disk (they can name local files). Path:
              <code class="break-all">{diag.engineLogPath}</code>
            </p>
          </details>
        {/if}
      </div>
    {/if}
  </section>
</div>

{#if info}
  <ModelInfoModal
    result={info}
    installing={!!modelDownload}
    onClose={() => (info = null)}
    onInstall={() => {
      const selected = info;
      if (!selected) return;
      installFrom(selected);
      info = null;
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
