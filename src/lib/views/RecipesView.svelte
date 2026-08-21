<script lang="ts">
  import { api, type Recipe } from "$lib/api";
  import { previewParts } from "$lib/placeholders";
  import { notifyInvokeError, startRecipe } from "$lib/stores.svelte";
  import { PROMPT_MAX_CHARS } from "$lib/text-cap";
  import {
    BookmarkPlus,
    ChefHat,
    Trash2,
    ArrowUpRight,
    RotateCcw,
    X,
    Pencil,
  } from "@lucide/svelte";
  import { confirmDanger } from "$lib/native-dialog";

  let recipes = $state<Recipe[]>([]);
  let formOpen = $state(false);
  let editingId = $state<string | null>(null);
  let formName = $state("");
  let formPrompt = $state("");

  async function refresh() {
    recipes = await api.recipesList();
  }

  $effect(() => {
    refresh().catch(notifyInvokeError);
  });

  function use(recipe: Recipe) {
    startRecipe(recipe.prompt);
  }

  function beginCreate() {
    editingId = null;
    formName = "";
    formPrompt = "";
    formOpen = true;
  }

  function beginEdit(recipe: Recipe, event: MouseEvent) {
    event.stopPropagation();
    editingId = recipe.id;
    formName = recipe.name;
    formPrompt = recipe.prompt;
    formOpen = true;
  }

  function cancelForm() {
    formOpen = false;
    editingId = null;
    formName = "";
    formPrompt = "";
  }

  async function remove(recipe: Recipe, event: MouseEvent) {
    event.stopPropagation();
    const ok = await confirmDanger(`Delete the “${recipe.name}” Recipe?`, "Delete");
    if (!ok) return;
    try {
      await api.recipeDelete(recipe.id);
      if (editingId === recipe.id) cancelForm();
      await refresh();
    } catch (error) {
      notifyInvokeError(error);
    }
  }

  async function save() {
    try {
      if (editingId) {
        await api.recipeUpdate(editingId, formName, formPrompt);
      } else {
        await api.recipeCreate(formName, formPrompt);
      }
      cancelForm();
      await refresh();
    } catch (error) {
      notifyInvokeError(error);
    }
  }

  async function restoreDefaults() {
    const ok = await confirmDanger(
      "Replace all Recipes with the defaults? Recipes you added or changed will be removed.",
      "Restore",
    );
    if (!ok) return;
    try {
      recipes = await api.recipesRestoreDefaults();
      cancelForm();
    } catch (error) {
      notifyInvokeError(error);
    }
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
    <button type="button" class="btn-primary" onclick={beginCreate}>
      <BookmarkPlus size={15} aria-hidden="true" /> New Recipe
    </button>
  </div>

  {#if formOpen}
    <div class="card mb-5 px-5 py-4">
      <div class="mb-3 flex items-center justify-between">
        <span class="text-[13.5px] font-semibold text-ink"
          >{editingId ? "Edit Recipe" : "New Recipe"}</span
        >
        <button
          type="button"
          class="btn-ghost !p-1.5"
          aria-label={editingId ? "Cancel edit Recipe" : "Cancel new Recipe"}
          onclick={cancelForm}><X size={14} aria-hidden="true" /></button
        >
      </div>
      <label class="sr-only" for="recipe-name">Recipe name</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        id="recipe-name"
        name="recipe-name"
        class="input mb-2.5"
        placeholder="Name, like Weekly team update"
        bind:value={formName}
        autofocus
      />
      <label class="sr-only" for="recipe-prompt">Recipe prompt</label>
      <textarea
        id="recipe-prompt"
        name="recipe-prompt"
        class="input min-h-28 cursor-text resize-y select-text"
        placeholder={"The prompt this Recipe starts with. Use «angle quotes» for the bits that change each time, e.g.\nSummarize what changed for «client name» this month and draft a short update email."}
        maxlength={PROMPT_MAX_CHARS}
        bind:value={formPrompt}></textarea>
      <div class="mt-2.5 flex justify-end gap-2">
        <button type="button" class="btn-ghost" onclick={cancelForm}>Cancel</button>
        <button
          type="button"
          class="btn-amber"
          onclick={save}
          disabled={!formName.trim() || !formPrompt.trim()}
        >
          Save Recipe
        </button>
      </div>
    </div>
  {/if}

  <div class="grid grid-cols-2 gap-4">
    {#each recipes as recipe (recipe.id)}
      <div
        class="card group relative flex flex-col px-5 py-4 text-left hover:shadow-pop dark:hover:shadow-none"
      >
        <div class="mb-1.5 flex items-center gap-2.5">
          <button
            type="button"
            class="flex min-w-0 flex-1 items-center gap-2.5 text-left"
            onclick={() => use(recipe)}
          >
            <span class="rounded-lg bg-navy-900 p-2 text-mint"
              ><ChefHat size={14} aria-hidden="true" /></span
            >
            <span class="flex-1 truncate text-[14px] font-semibold text-ink">{recipe.name}</span>
            <span class="rounded-md p-1.5 text-navy-600 dark:text-navy-300" title="Use this Recipe">
              <ArrowUpRight size={14} aria-hidden="true" />
            </span>
          </button>
          <button
            type="button"
            class="rounded-md p-1.5 text-ink-faint hover:bg-navy-50 hover:text-ink dark:hover:bg-white/6"
            aria-label="Edit Recipe"
            title="Edit Recipe"
            onclick={(e) => beginEdit(recipe, e)}
          >
            <Pencil size={13} aria-hidden="true" />
          </button>
          <button
            type="button"
            class="rounded-md p-1.5 text-ink-faint hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-400/10 dark:hover:text-red-400"
            aria-label="Delete Recipe"
            title="Delete Recipe"
            onclick={(e) => remove(recipe, e)}
          >
            <Trash2 size={13} aria-hidden="true" />
          </button>
        </div>
        <button
          type="button"
          class="line-clamp-3 text-left text-[12px] leading-relaxed text-ink-soft"
          aria-label="Use {recipe.name}"
          onclick={() => use(recipe)}
        >
          {#each previewParts(recipe.prompt) as part}
            {#if part.ph}<span
                class="rounded-md bg-navy-100 px-1 py-px font-medium text-navy-800 dark:bg-navy-500/15 dark:text-navy-200 dark:inset-ring dark:inset-ring-navy-500/45"
                >{part.text}</span
              >{:else}{part.text}{/if}
          {/each}
        </button>
      </div>
    {:else}
      <div class="card col-span-2 flex flex-col items-center px-8 py-12 text-center">
        <ChefHat size={22} class="mb-2 text-ink-faint" aria-hidden="true" />
        <p class="text-[13.5px] font-medium text-ink">No Recipes yet</p>
        <p class="mt-1 text-[12.5px] text-ink-soft">
          Save a prompt you repeat, or bring back the defaults.
        </p>
      </div>
    {/each}
  </div>

  <div class="mt-5 flex justify-center">
    <button type="button" class="btn-ghost !px-3 !py-1.5 !text-[12px]" onclick={restoreDefaults}>
      <RotateCcw size={12.5} aria-hidden="true" /> Restore default Recipes
    </button>
  </div>
</div>
