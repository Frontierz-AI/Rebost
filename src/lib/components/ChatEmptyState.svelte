<script lang="ts">
  import { api, type Recipe } from "$lib/api";
  import { promptNeedsShelf } from "$lib/placeholders";
  import { app, chatState, fillDraft } from "$lib/stores.svelte";
  import mark from "../../assets/R.webp";

  let recipes = $state<Recipe[]>([]);
  const hasModel = $derived(!!app.settings?.activeModel);

  $effect(() => {
    if (!hasModel) {
      recipes = [];
      return;
    }
    api
      .recipesList()
      .then((list) => (recipes = list))
      .catch(() => {
        recipes = [];
      });
  });

  const shown = $derived.by(() => {
    const hasShelf = !!chatState.selectedShelfId;
    const withShelf = recipes.filter((recipe) => promptNeedsShelf(recipe.prompt));
    const without = recipes.filter((recipe) => !promptNeedsShelf(recipe.prompt));
    const ordered = hasShelf ? [...withShelf, ...without] : without;
    return ordered.slice(0, 6);
  });
</script>

<div class="flex h-full flex-col items-center justify-center px-8">
  <img src={mark} alt="Rebost" class="mb-3 w-[100px] rounded-2xl" />
  {#if !hasModel}
    <h2 class="text-[19px] font-semibold text-ink">Install an AI first</h2>
    <p class="mt-1 mb-6 max-w-md text-center text-[13px] text-ink-soft">
      Chat needs one on this computer before it can answer.
    </p>
    <button
      type="button"
      class="btn-amber focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy-400"
      onclick={() => (app.view = "settings")}
    >
      Install
    </button>
  {:else}
    <h2 class="text-[19px] font-semibold text-ink">Start a conversation</h2>
    <p class="mt-1 mb-6 max-w-md text-center text-[13px] text-ink-soft">
      Ask a question. Choose a Shelf when the answer should come from your files.
    </p>
  {/if}
  {#if hasModel && shown.length > 0}
    <div class="grid w-full max-w-xl grid-cols-2 gap-2">
      {#each shown as recipe (recipe.id)}
        <button
          type="button"
          class="card px-3.5 py-3 text-left text-[12.5px] text-ink-soft hover:border-navy-300 hover:text-ink"
          onclick={() => fillDraft(recipe.prompt)}
        >
          {recipe.name}
        </button>
      {/each}
    </div>
  {:else if hasModel && recipes.length > 0 && !chatState.selectedShelfId && app.shelves.length > 0}
    <p class="max-w-md text-center text-[12.5px] text-ink-faint">
      Choose a Shelf to see Recipes that use your files.
    </p>
  {/if}
</div>
