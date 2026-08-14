<script lang="ts">
  import { api, type Recipe } from "$lib/api";
  import { notify, startRecipe } from "$lib/stores.svelte";
  import { BookmarkPlus, ChefHat, Trash2, ArrowUpRight, RotateCcw, X } from "@lucide/svelte";

  let recipes = $state<Recipe[]>([]);
  let creating = $state(false);
  let newName = $state("");
  let newPrompt = $state("");

  async function refresh() {
    recipes = await api.recipesList();
  }

  $effect(() => {
    refresh();
  });

  const missingDefaults = $derived(recipes.filter((r) => r.builtin).length < 8);

  function use(recipe: Recipe) {
    startRecipe(recipe.prompt);
  }

  async function remove(recipe: Recipe, event: MouseEvent) {
    event.stopPropagation();
    if (!confirm(`Delete the “${recipe.name}” Recipe?`)) return;
    try {
      await api.recipeDelete(recipe.id);
      await refresh();
    } catch (error) {
      notify(String(error));
    }
  }

  async function create() {
    try {
      await api.recipeCreate(newName, newPrompt);
      newName = "";
      newPrompt = "";
      creating = false;
      await refresh();
    } catch (error) {
      notify(String(error));
    }
  }

  async function restoreDefaults() {
    recipes = await api.recipesRestoreDefaults();
  }

  /// Show «placeholders» with a subtle highlight in the preview.
  function previewParts(prompt: string): { text: string; ph: boolean }[] {
    const clean = prompt.replace(/\s+/g, " ").trim();
    const parts: { text: string; ph: boolean }[] = [];
    const re = /«[^»]*»/g;
    let last = 0;
    for (const match of clean.matchAll(re)) {
      if (match.index! > last) parts.push({ text: clean.slice(last, match.index), ph: false });
      parts.push({ text: match[0], ph: true });
      last = match.index! + match[0].length;
    }
    if (last < clean.length) parts.push({ text: clean.slice(last), ph: false });
    return parts;
  }
</script>

<div class="mx-auto max-w-[860px] px-8 py-8">
  <div class="mb-6 flex items-end justify-between">
    <div>
      <h1 class="text-[22px] font-semibold text-ink">Recipes</h1>
      <p class="mt-0.5 text-[13px] text-ink-soft">
        Saved prompts. Click one and a new conversation opens with the prompt already in the box.
      </p>
    </div>
    <button type="button" class="btn-primary" onclick={() => (creating = true)}>
      <BookmarkPlus size={15} /> New Recipe
    </button>
  </div>

  {#if creating}
    <div class="card mb-5 px-5 py-4">
      <div class="mb-3 flex items-center justify-between">
        <span class="text-[13.5px] font-semibold text-ink">New Recipe</span>
        <button
          type="button"
          class="btn-ghost !p-1.5"
          aria-label="Cancel new Recipe"
          onclick={() => (creating = false)}><X size={14} /></button
        >
      </div>
      <!-- svelte-ignore a11y_autofocus -->
      <label class="sr-only" for="recipe-name">Recipe name</label>
      <input
        id="recipe-name"
        class="input mb-2.5"
        placeholder="Name, like Weekly team update"
        bind:value={newName}
        autofocus
      />
      <label class="sr-only" for="recipe-prompt">Recipe prompt</label>
      <textarea
        id="recipe-prompt"
        class="input min-h-28 resize-y select-text cursor-text"
        placeholder={"The prompt this Recipe starts with. Use «angle quotes» for the bits that change each time, e.g.\nSummarize what changed for «client name» this month and draft a short update email."}
        bind:value={newPrompt}></textarea>
      <div class="mt-2.5 flex justify-end gap-2">
        <button type="button" class="btn-ghost" onclick={() => (creating = false)}>Cancel</button>
        <button
          type="button"
          class="btn-amber"
          onclick={create}
          disabled={!newName.trim() || !newPrompt.trim()}
        >
          Save Recipe
        </button>
      </div>
    </div>
  {/if}

  <div class="grid grid-cols-2 gap-4">
    {#each recipes as recipe (recipe.id)}
      <div class="card group relative flex flex-col px-5 py-4 text-left hover:shadow-pop">
        <div class="mb-1.5 flex items-center gap-2.5">
          <button
            type="button"
            class="flex min-w-0 flex-1 items-center gap-2.5 text-left"
            onclick={() => use(recipe)}
          >
            <span class="rounded-lg bg-navy-900 p-2 text-amber-450"><ChefHat size={14} /></span>
            <span class="flex-1 truncate text-[14px] font-semibold text-ink">{recipe.name}</span>
            <span class="rounded-md p-1.5 text-navy-600" title="Use this Recipe">
              <ArrowUpRight size={14} />
            </span>
          </button>
          <button
            type="button"
            class="rounded-md p-1.5 text-ink-faint hover:bg-red-50 hover:text-red-700"
            aria-label="Delete Recipe"
            title="Delete Recipe"
            onclick={(e) => remove(recipe, e)}
          >
            <Trash2 size={13} />
          </button>
        </div>
        <button
          type="button"
          class="line-clamp-3 text-left text-[12px] leading-relaxed text-ink-soft"
          onclick={() => use(recipe)}
        >
          {#each previewParts(recipe.prompt) as part}
            {#if part.ph}<span class="rounded bg-amber-350/40 px-1 py-px font-medium text-amber-550"
                >{part.text}</span
              >{:else}{part.text}{/if}
          {/each}
        </button>
      </div>
    {:else}
      <div class="card col-span-2 flex flex-col items-center px-8 py-12 text-center">
        <ChefHat size={22} class="mb-2 text-ink-faint" />
        <p class="text-[13.5px] font-medium text-ink">No Recipes yet</p>
        <p class="mt-1 text-[12.5px] text-ink-soft">
          Save a prompt you repeat, or bring back the defaults.
        </p>
      </div>
    {/each}
  </div>

  {#if missingDefaults}
    <div class="mt-5 flex justify-center">
      <button type="button" class="btn-ghost !text-[12px]" onclick={restoreDefaults}>
        <RotateCcw size={12.5} /> Restore default Recipes
      </button>
    </div>
  {/if}
</div>
