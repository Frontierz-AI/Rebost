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
  import { app, beginModelInstall, notify, refreshSettings } from "$lib/stores.svelte";
  import {
    Cpu,
    Search,
    Download,
    BadgeCheck,
    Stethoscope,
    Save,
    Info,
  } from "@lucide/svelte";
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
  let rulesSaved = $state(false);
  let diag = $state<Diagnostics | null>(null);
  let showDiag = $state(false);
  let showReset = $state(false);
  let resetting = $state(false);

  $effect(() => {
    void app.settings?.activeModel?.reference;
    api.machineProfile().then((m) => (machine = m));
  });

  $effect(() => {
    houseRules = app.settings?.houseRules ?? "";
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
      notify(String(error));
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
    await api.setHouseRules(houseRules);
    await refreshSettings();
    rulesSaved = true;
    setTimeout(() => (rulesSaved = false), 1500);
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
      notify(String(error));
    }
  }
</script>

<div class="mx-auto max-w-[760px] px-8 py-8">
  <h1 class="mb-6 text-[22px] font-semibold text-ink">Settings</h1>

  <!-- ── AI model ────────────────────────────────────────────────────── -->
  <section class="card mb-6 px-6 py-5">
    <h2 class="mb-1 text-[15px] font-semibold text-ink">AI model</h2>
    {#if machine}
      <p class="mb-4 flex items-center gap-1.5 text-[12px] text-ink-faint">
        <Cpu size={12.5} />
        {machine.profile.cpu} · {formatBytes(machine.profile.totalRamBytes)} memory ·
        {formatBytes(machine.profile.freeDiskBytes)} free disk
      </p>
    {/if}

    {#if modelDownload}
      <div class="mb-4 rounded-xl border border-navy-200 bg-navy-50/60 px-4 py-3">
        <DownloadProgress download={modelDownload} cancelable />
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
            ? "Other models that fit"
            : "Suggested for this computer"}
          lede={app.settings?.activeModel
            ? "Catalog picks you have not installed, sized for this computer."
            : "Picked for the RAM on this computer."}
          oninstall={installRec}
        />
      </div>
    {/if}

    <!-- Explore other models -->
    <div class="mt-5">
      <h3 class="label mb-2">Explore other models</h3>
      <div class="flex gap-2">
        <div class="relative flex-1">
          <Search size={13.5} class="absolute top-1/2 left-3 -translate-y-1/2 text-ink-faint" />
          <label class="sr-only" for="model-search">Search models</label>
          <input
            id="model-search"
            class="input !pl-9"
            placeholder="Search models: Muse, Gemma, Mistral…"
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
              class="flex items-center gap-3 border-b border-paper-line/70 bg-white px-4 py-2.5 last:border-b-0"
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
            <p class="bg-white px-4 py-3 text-[12.5px] text-ink-soft">
              Nothing found for that search.
            </p>
          {/each}
        </div>
        {#if results.length > 0}
          <p class="mt-1.5 text-[11px] text-ink-faint">
            Rebost keeps one model installed at a time and picks a GGUF that should fit this
            machine.
          </p>
        {/if}
      {/if}
    </div>
  </section>

  <!-- ── House rules ─────────────────────────────────────────────────── -->
  <section class="card mb-6 px-6 py-5">
    <h2 class="mb-1 text-[15px] font-semibold text-ink">House rules</h2>
    <p class="mb-3 text-[12.5px] leading-snug text-ink-soft">
      Standing instructions: tone, language, what never to promise. Rebost follows them in every
      conversation and Recipe.
    </p>
    <label class="sr-only" for="house-rules">House rules</label>
    <textarea
      id="house-rules"
      class="input min-h-32 resize-y select-text cursor-text"
      placeholder={"Examples:\n· Answer in the language of the documents unless asked otherwise.\n· We are a furniture workshop. Use first names.\n· Never promise delivery dates; say the team will confirm."}
      bind:value={houseRules}></textarea>
    <div class="mt-2.5 flex justify-end">
      <button type="button" class="btn-primary" onclick={saveRules}>
        <Save size={13.5} />
        {rulesSaved ? "Saved" : "Save house rules"}
      </button>
    </div>
  </section>

  <!-- ── About ──────────────────────────────────────────────────────── -->
  <section class="card mb-6 px-6 py-5">
    <div class="flex items-center gap-4">
      <img src={icon} alt="" class="h-12 w-12 rounded-[22%]" />
      <div class="min-w-0 flex-1">
        <h2 class="text-[15px] font-semibold text-ink">About Rebost</h2>
        <p class="mt-0.5 text-[12.5px] leading-snug text-ink-soft">
          Private AI that lives with your files and never leaves them.
        </p>
      </div>
      <button
        type="button"
        class="btn-outline shrink-0"
        onclick={() => api.showAboutWindow().catch((error) => notify(String(error)))}
      >
        Open
      </button>
    </div>
  </section>

  <!-- ── Reset ──────────────────────────────────────────────────────── -->
  <section class="card mb-6 px-6 py-5">
    <div class="flex items-center gap-4">
      <div class="min-w-0 flex-1">
        <h2 class="text-[15px] font-semibold text-ink">Reset Rebost</h2>
        <p class="mt-0.5 text-[12.5px] leading-snug text-ink-soft">
          Deletes conversations, settings, House rules, Recipes, the search index, the installed
          model, and the AI engine. Your own files stay on disk. Rebost forgets the Shelves that
          pointed at them.
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

  <!-- ── Diagnostics (outside the normal UI) ─────────────────────────── -->
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
    onclose={() => (info = null)}
    oninstall={() => {
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
    onclose={() => {
      if (!resetting) showReset = false;
    }}
    onconfirm={resetWorkspace}
  />
{/if}
