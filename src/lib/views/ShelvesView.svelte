<script lang="ts">
  import { api, piiLabel, type Card, type DocumentMeta, type ShelfView } from "$lib/api";
  import { app, notify, refreshShelves } from "$lib/stores.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    LibraryBig,
    Plus,
    FolderSymlink,
    FolderInput,
    FolderPlus,
    X,
    Trash2,
    ShieldCheck,
    ChevronLeft,
    Search,
  } from "@lucide/svelte";
  import DocumentDrawer from "$lib/components/DocumentDrawer.svelte";
  import ShelfDocumentTable from "$lib/components/ShelfDocumentTable.svelte";

  let creating = $state(app.createShelf);
  if (app.createShelf) app.createShelf = false;

  let newName = $state("");
  let documents = $state<DocumentMeta[]>([]);
  let dropActive = $state(false);
  let filterPii = $state<string | null>(null);
  let filterStatus = $state<string | null>(null);
  let filterSource = $state<string | null>(null);
  let filterType = $state<string | null>(null);
  let searchText = $state("");
  let openDoc = $state<DocumentMeta | null>(null);
  let openCard = $state<Card | null>(null);
  let extractedText = $state<string | null>(null);

  const shelf = $derived(app.shelves.find((s) => s.id === app.openShelfId) ?? null);

  // Reload documents when the shelf changes or ingestion progresses.
  $effect(() => {
    void app.ingestTick;
    const id = app.openShelfId;
    if (id) {
      api.shelfDocuments(id).then((docs) => (documents = docs));
    } else {
      documents = [];
    }
  });

  // Native drag & drop of files onto the shelf detail.
  $effect(() => {
    if (!shelf) return;
    const shelfId = shelf.id;
    let unlisten: (() => void) | null = null;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          dropActive = true;
        } else if (event.payload.type === "drop") {
          dropActive = false;
          const paths = event.payload.paths;
          if (paths.length > 0) {
            api
              .shelfImportPaths(shelfId, paths)
              .then((count) => {
                if (count === 0) notify("Those files aren't a supported type.");
              })
              .catch((error) => notify(String(error)));
          }
        } else {
          dropActive = false;
        }
      })
      .then((fn) => (unlisten = fn));
    return () => unlisten?.();
  });

  const fileTypes = $derived([...new Set(documents.map((d) => d.format.toUpperCase()))].sort());
  const sourceLabels = $derived([...new Set(documents.map((d) => d.sourceLabel))].sort());

  const visibleDocs = $derived(
    documents.filter((doc) => {
      if (filterPii === "any" && doc.piiTotal === 0) return false;
      if (filterPii && filterPii !== "any" && !(doc.piiCategories?.[filterPii] ?? 0)) return false;
      if (filterStatus && doc.status !== filterStatus) return false;
      if (filterSource && doc.sourceLabel !== filterSource) return false;
      if (filterType && doc.format.toUpperCase() !== filterType) return false;
      if (searchText && !doc.fileName.toLowerCase().includes(searchText.toLowerCase()))
        return false;
      return true;
    }),
  );

  async function createShelf() {
    const name = newName.trim();
    if (!name) return;
    try {
      const created = await api.shelfCreate(name);
      newName = "";
      creating = false;
      await refreshShelves();
      app.openShelfId = created.id;
    } catch (error) {
      notify(String(error));
    }
  }

  async function openFile(doc: DocumentMeta) {
    openDoc = doc;
    openCard = null;
    extractedText = null;
    if (doc.status === "ready") {
      try {
        openCard = await api.documentCard(doc.shelfId, doc.id);
      } catch {
        openCard = null;
      }
    }
  }

  async function unlinkSource(shelfView: ShelfView, sourceId: string) {
    await api.shelfRemoveSource(shelfView.id, sourceId);
    await refreshShelves();
  }

  async function deleteShelf(shelfView: ShelfView, event: MouseEvent) {
    event.stopPropagation();
    if (!confirm(`Delete the “${shelfView.name}” Shelf from Rebost? Files on disk are kept.`)) {
      return;
    }
    try {
      await api.shelfRemove(shelfView.id);
      if (app.openShelfId === shelfView.id) app.openShelfId = null;
      await refreshShelves();
    } catch (error) {
      notify(String(error));
    }
  }

  function onRowKey(event: KeyboardEvent, doc: DocumentMeta) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      openFile(doc);
    }
  }

  async function addFiles() {
    if (!shelf) return;
    try {
      await api.shelfImportDialog(shelf.id);
    } catch (error) {
      notify(String(error));
    }
  }

  async function addFolder() {
    if (!shelf) return;
    try {
      await api.shelfAddLinked(shelf.id);
      await refreshShelves();
    } catch (error) {
      notify(String(error));
    }
  }

  function clearFilters() {
    filterPii = null;
    filterStatus = null;
    filterSource = null;
    filterType = null;
    searchText = "";
  }

  const filtersOn = $derived(
    !!(filterPii || filterStatus || filterSource || filterType || searchText),
  );

  const PII_ORDER = ["email", "nif", "nie", "iban", "phone", "credit_card", "ip_address"];
</script>

{#if !shelf}
  <!-- ── Shelf list ─────────────────────────────────────────────────── -->
  <div class="h-full overflow-y-auto">
    <div class="mx-auto max-w-[860px] px-8 py-8">
      <div class="mb-6 flex items-end justify-between">
        <div>
          <h1 class="text-[22px] font-semibold text-ink">Shelves</h1>
          <p class="mt-0.5 text-[13px] text-ink-soft">
            Files Rebost can answer from. Citations open the source.
          </p>
        </div>
        <button type="button" class="btn-primary" onclick={() => (creating = true)}>
          <Plus size={15} /> New Shelf
        </button>
      </div>

      {#if creating}
        <div class="card mb-5 flex items-center gap-2 px-4 py-3">
          <LibraryBig size={16} class="text-navy-500" />
          <label class="sr-only" for="new-shelf-name">Shelf name</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="new-shelf-name"
            class="w-full rounded-none border-none bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-faint select-text cursor-text"
            placeholder="Shelf name, like Projects or Research"
            bind:value={newName}
            autofocus
            onkeydown={(e) => e.key === "Enter" && createShelf()}
          />
          <button type="button" class="btn-amber !py-1.5" onclick={createShelf}>Create</button>
          <button type="button" class="btn-ghost !py-1.5" onclick={() => (creating = false)}
            >Cancel</button
          >
        </div>
      {/if}

      {#if app.shelves.length === 0 && !creating}
        <div class="card flex flex-col items-center px-8 py-14 text-center">
          <div class="mb-3 rounded-2xl bg-navy-100 p-3.5 text-navy-700">
            <LibraryBig size={24} />
          </div>
          <h2 class="text-[16px] font-semibold text-ink">No Shelves yet</h2>
          <p class="mt-1 mb-5 max-w-sm text-[13px] text-ink-soft">
            Chat works without a Shelf. Add one and drop files on it when you want answers from your
            own documents.
          </p>
          <button type="button" class="btn-primary" onclick={() => (creating = true)}
            ><Plus size={15} /> Create your first Shelf</button
          >
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-4">
          {#each app.shelves as shelfCard (shelfCard.id)}
            <div class="card group flex items-stretch hover:shadow-pop">
              <button
                type="button"
                class="min-w-0 flex-1 px-5 py-4 text-left"
                onclick={() => (app.openShelfId = shelfCard.id)}
              >
                <div class="flex items-center gap-3">
                  <span class="rounded-xl bg-navy-900 p-2.5 text-amber-450"
                    ><LibraryBig size={18} /></span
                  >
                  <span class="min-w-0">
                    <span class="block truncate text-[15px] font-semibold text-ink"
                      >{shelfCard.name}</span
                    >
                    <span class="block text-[12px] text-ink-soft">
                      {shelfCard.stats.files} files · {shelfCard.stats.searchable} searchable
                      {#if shelfCard.stats.reading > 0}
                        · <span class="text-amber-550">{shelfCard.stats.reading} reading</span>
                      {/if}
                    </span>
                  </span>
                </div>
                {#if shelfCard.stats.pii.total > 0}
                  <p class="mt-3 flex items-center gap-1.5 text-[11.5px] text-ink-faint">
                    <ShieldCheck size={12.5} class="text-navy-500" />
                    {shelfCard.stats.filesWithPii} files contain personal information
                  </p>
                {/if}
              </button>
              <button
                type="button"
                class="btn-ghost m-3 self-start !p-1.5"
                aria-label="Delete Shelf"
                onclick={(e) => deleteShelf(shelfCard, e)}
              >
                <Trash2 size={14} />
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{:else}
  <!-- ── Shelf detail ───────────────────────────────────────────────── -->
  <div class="relative flex h-full min-h-0 flex-col overflow-hidden">
    {#if dropActive && documents.length > 0}
      <div
        class="pointer-events-none absolute inset-3 z-30 flex items-center justify-center rounded-2xl border-2 border-dashed border-amber-450 bg-amber-350/20"
      >
        <p class="rounded-xl bg-navy-900 px-4 py-2 text-[13.5px] font-medium text-white shadow-pop">
          Drop files to add them to {shelf.name}
        </p>
      </div>
    {/if}

    <header class="flex shrink-0 flex-col gap-3 px-8 pt-5 pb-5">
      <button
        type="button"
        class="inline-flex min-h-8 w-fit items-center gap-1 rounded-md text-[13px] font-medium text-ink-soft hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-500"
        onclick={() => (app.openShelfId = null)}
      >
        <ChevronLeft size={16} class="shrink-0 -ml-0.5" aria-hidden="true" />
        All Shelves
      </button>

      <div class="flex items-start justify-between gap-4">
        <div class="flex min-w-0 flex-col gap-1.5">
          <h1 class="truncate text-[1.375rem] font-semibold tracking-tight text-navy-950">
            {shelf.name}
          </h1>
          <p class="flex flex-wrap items-center gap-x-2 text-[13px] text-ink-soft">
            <span class="font-medium tabular-nums text-ink">{shelf.stats.files} files</span>
            <span aria-hidden="true">·</span>
            <button
              type="button"
              class="tabular-nums hover:text-ink aria-pressed:text-navy-800 aria-pressed:underline aria-pressed:underline-offset-4"
              aria-pressed={filterStatus === "ready"}
              onclick={() => (filterStatus = filterStatus === "ready" ? null : "ready")}
            >
              {shelf.stats.searchable} searchable
            </button>
            {#if shelf.stats.reading > 0}
              <span aria-hidden="true">·</span>
              <button
                type="button"
                class="inline-flex items-center gap-1.5 tabular-nums text-amber-550 aria-pressed:underline aria-pressed:underline-offset-4"
                aria-pressed={filterStatus === "reading"}
                onclick={() => (filterStatus = filterStatus === "reading" ? null : "reading")}
              >
                <span
                  class="inline-block size-1.5 animate-pulse rounded-full bg-amber-450"
                  aria-hidden="true"
                ></span>
                {shelf.stats.reading} reading
              </button>
            {/if}
            {#if shelf.stats.errors > 0}
              <span aria-hidden="true">·</span>
              <button
                type="button"
                class="tabular-nums text-red-700/80 hover:text-red-800 aria-pressed:underline aria-pressed:underline-offset-4"
                aria-pressed={filterStatus === "error"}
                onclick={() => (filterStatus = filterStatus === "error" ? null : "error")}
              >
                {shelf.stats.errors} couldn't be read
              </button>
            {/if}
          </p>
        </div>

        <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
          {#if documents.length > 0}
            <button type="button" class="btn-primary py-2 pr-3 pl-2" onclick={addFiles}>
              <FolderInput size={14} class="shrink-0" aria-hidden="true" />
              Add files
            </button>
            <button
              type="button"
              class="btn-outline py-2 pr-3 pl-2"
              title="Link a folder on this computer. New files in it show up here."
              onclick={addFolder}
            >
              <FolderSymlink size={14} class="shrink-0" aria-hidden="true" />
              Add folder
            </button>
          {/if}
          <button
            type="button"
            class="btn-ghost py-2 pr-3 pl-2"
            aria-label="Delete Shelf"
            onclick={(e) => deleteShelf(shelf, e)}
          >
            <Trash2 size={14} class="shrink-0" aria-hidden="true" />
            Delete
          </button>
        </div>
      </div>

      {#if shelf.stats.pii.total > 0}
        <div class="flex flex-wrap items-center gap-1.5">
          <button
            type="button"
            class="chip py-1 pr-2.5 pl-1.5 {filterPii === 'any'
              ? 'bg-navy-900 text-white'
              : 'bg-navy-100/70 text-navy-800 hover:bg-navy-200/70'}"
            aria-pressed={filterPii === "any"}
            onclick={() => (filterPii = filterPii === "any" ? null : "any")}
          >
            <ShieldCheck size={12} class="shrink-0" aria-hidden="true" />
            {shelf.stats.filesWithPii} files contain personal information
          </button>
          {#each PII_ORDER.filter((c) => shelf.stats.pii.categories[c]) as category}
            {@const count = shelf.stats.pii.categories[category] ?? 0}
            <button
              type="button"
              class="chip {filterPii === category
                ? 'bg-navy-900 text-white'
                : 'bg-paper-soft text-ink-soft hover:bg-navy-100/70 hover:text-navy-800'}"
              aria-pressed={filterPii === category}
              onclick={() => (filterPii = filterPii === category ? null : category)}
            >
              {count}
              {piiLabel(category, count)}
            </button>
          {/each}
        </div>
      {/if}

      {#if shelf.linkedFolders.length > 0}
        <div class="flex flex-wrap items-center gap-1.5">
          <p class="label !text-[10px]">Linked folders</p>
          {#each shelf.linkedFolders as linked (linked.sourceId)}
            <span
              class="chip !cursor-default border border-paper-line bg-white py-1 pr-1 pl-1.5 text-ink-soft"
            >
              <FolderSymlink size={11.5} class="shrink-0 text-navy-500" aria-hidden="true" />
              <span class="max-w-[260px] truncate" title={linked.path}>{linked.path}</span>
              <button
                type="button"
                class="rounded p-0.5 hover:bg-red-50 hover:text-red-700"
                aria-label="Remove linked folder from this Shelf"
                title="Remove from this Shelf (files stay on disk)"
                onclick={() => unlinkSource(shelf, linked.sourceId)}
              >
                <X size={11} aria-hidden="true" />
              </button>
            </span>
          {/each}
        </div>
      {/if}
    </header>

    {#if documents.length === 0}
      <div class="flex min-h-0 flex-1 flex-col px-8 pb-8">
        <div
          class="flex min-h-0 flex-1 flex-col items-center justify-center gap-5 rounded-xl border-2 border-dashed px-8 py-10 text-center {dropActive
            ? 'border-amber-450 bg-amber-350/20'
            : 'border-paper-line bg-paper-soft/40'}"
          role="region"
          aria-label="Empty Shelf"
        >
          <FolderPlus size={24} class="shrink-0 text-navy-500" aria-hidden="true" />
          <div class="flex flex-col items-center gap-1">
            <p class="text-[0.9375rem] font-medium text-ink">Drop files to fill this Shelf</p>
            <p class="max-w-[42ch] text-pretty text-[13px] text-ink-soft">
              PDFs, Office documents, spreadsheets, presentations, and email. Rebost reads them on
              this computer.
            </p>
          </div>
          <div class="flex flex-wrap items-center justify-center gap-2">
            <button type="button" class="btn-primary py-2 pr-3 pl-2" onclick={addFiles}>
              <FolderInput size={14} class="shrink-0" aria-hidden="true" />
              Add files
            </button>
            <button
              type="button"
              class="btn-outline py-2 pr-3 pl-2"
              title="Link a folder on this computer. New files in it show up here."
              onclick={addFolder}
            >
              <FolderSymlink size={14} class="shrink-0" aria-hidden="true" />
              Add folder from this computer
            </button>
          </div>
        </div>
      </div>
    {:else}
      <div class="flex min-h-0 flex-1 flex-col">
        <div
          class="flex shrink-0 items-center justify-between gap-3 border-t border-paper-line px-8 py-2.5"
        >
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <div class="relative min-w-0">
              <Search
                size={13}
                class="pointer-events-none absolute top-1/2 left-2.5 shrink-0 -translate-y-1/2 text-ink-faint"
                aria-hidden="true"
              />
              <label class="sr-only" for="shelf-filter-name">Filter by file name</label>
              <input
                id="shelf-filter-name"
                name="filter-name"
                type="text"
                autocomplete="off"
                class="input !w-56 !py-1.5 !pl-8 !text-[12.5px]"
                placeholder="Filter by name"
                bind:value={searchText}
              />
            </div>
            <label class="sr-only" for="shelf-filter-source">Filter by source</label>
            <span class="inline-grid grid-cols-[1fr_--spacing(8)]">
              <select
                id="shelf-filter-source"
                name="filter-source"
                class="input col-span-full row-start-1 appearance-none !w-auto !py-1.5 !pr-8 !text-[12.5px]"
                bind:value={filterSource}
              >
                <option value={null}>All sources</option>
                {#each sourceLabels as label}<option value={label}>{label}</option>{/each}
              </select>
              <svg
                viewBox="0 0 8 5"
                width="8"
                height="5"
                fill="none"
                class="pointer-events-none col-start-2 row-start-1 place-self-center"
                aria-hidden="true"
              >
                <path d="M.5.5 4 4 7.5.5" stroke="currentcolor" />
              </svg>
            </span>
            <label class="sr-only" for="shelf-filter-type">Filter by type</label>
            <span class="inline-grid grid-cols-[1fr_--spacing(8)]">
              <select
                id="shelf-filter-type"
                name="filter-type"
                class="input col-span-full row-start-1 appearance-none !w-auto !py-1.5 !pr-8 !text-[12.5px]"
                bind:value={filterType}
              >
                <option value={null}>All types</option>
                {#each fileTypes as type}<option value={type}>{type}</option>{/each}
              </select>
              <svg
                viewBox="0 0 8 5"
                width="8"
                height="5"
                fill="none"
                class="pointer-events-none col-start-2 row-start-1 place-self-center"
                aria-hidden="true"
              >
                <path d="M.5.5 4 4 7.5.5" stroke="currentcolor" />
              </svg>
            </span>
            {#if filtersOn}
              <button type="button" class="btn-ghost !py-1 !text-[12px]" onclick={clearFilters}>
                <X size={12} class="shrink-0" aria-hidden="true" />
                Clear filters
              </button>
            {/if}
          </div>
          <p class="shrink-0 text-[11.5px] tabular-nums text-ink-faint" aria-live="polite">
            {visibleDocs.length} of {documents.length}
          </p>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto">
          <ShelfDocumentTable
            {visibleDocs}
            onopen={openFile}
            onrowkey={onRowKey}
            onclear={clearFilters}
          />
        </div>
      </div>
    {/if}
  </div>
{/if}

{#if openDoc}
  <DocumentDrawer
    doc={openDoc}
    card={openCard}
    bind:extractedText
    onclose={() => (openDoc = null)}
  />
{/if}
