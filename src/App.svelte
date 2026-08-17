<script lang="ts">
  import { onMount } from "svelte";
  import { Toaster, toast } from "svelte-sonner";
  import { fade } from "svelte/transition";
  import {
    app,
    bootstrap,
    handleMenuAction,
    notifyInvokeError,
    setNotifier,
    type View,
  } from "$lib/stores.svelte";
  import { api, events } from "$lib/api";
  import { motionMs } from "$lib/motion";
  import { parseMenuAction, shortcutAction } from "$lib/shortcuts";
  import { colorScheme, onColorSchemeChange } from "$lib/appearance";
  import { isMac } from "$lib/platform";
  import ChatView from "$lib/views/ChatView.svelte";
  import Onboarding from "$lib/views/Onboarding.svelte";
  import { MessageCircle, LibraryBig, ChefHat, Settings, ArrowUpCircle } from "@lucide/svelte";
  import icon from "./assets/rebost-icon.png";

  // Chat is home; other panes are separate chunks so first paint stays small.
  const loadShelves = () => import("$lib/views/ShelvesView.svelte");
  const loadRecipes = () => import("$lib/views/RecipesView.svelte");
  const loadSettings = () => import("$lib/views/SettingsView.svelte");

  const mac = isMac();
  let toastTheme = $state(colorScheme());
  // Overlay title bar on macOS: content is full height and the traffic lights
  // sit on the sidebar, so it needs top clearance and drag regions.

  onMount(() => {
    setNotifier((message) => toast(message));
    bootstrap().catch((error) => {
      console.error(error);
      toast("Rebost couldn't finish starting. Try again.");
    });

    function onKey(event: KeyboardEvent) {
      const action = shortcutAction(event);
      if (!action) return;
      event.preventDefault();
      handleMenuAction(action);
    }

    window.addEventListener("keydown", onKey);
    const menu = events.menu((payload) => {
      const action = parseMenuAction(payload.action);
      if (action) handleMenuAction(action);
    });
    const stopTheme = onColorSchemeChange((scheme) => {
      toastTheme = scheme;
    });

    return () => {
      window.removeEventListener("keydown", onKey);
      void menu.then((unlisten) => unlisten());
      stopTheme();
    };
  });

  const nav = [
    { view: "chat", label: "Chat", icon: MessageCircle },
    { view: "shelves", label: "Shelves", icon: LibraryBig },
    { view: "recipes", label: "Recipes", icon: ChefHat },
  ] as const;

  function pane(view: View): View {
    switch (view) {
      case "chat":
      case "shelves":
      case "recipes":
      case "settings":
        return view;
      default: {
        const _exhaustive: never = view;
        return _exhaustive;
      }
    }
  }

  const current = $derived(pane(app.view));
  let allowViewFade = $state(false);

  $effect(() => {
    if (!app.ready || app.onboarding) {
      allowViewFade = false;
      return;
    }
    const frame = requestAnimationFrame(() => {
      allowViewFade = true;
    });
    return () => cancelAnimationFrame(frame);
  });
</script>

<Toaster position="top-right" theme={toastTheme} richColors closeButton offset={mac ? 52 : 16} />

{#if !app.ready}
  <div class="flex h-full flex-col bg-navy-950">
    {#if mac}
      <div data-tauri-drag-region class="h-[46px] shrink-0"></div>
    {/if}
    <div data-tauri-drag-region class="flex flex-1 items-center justify-center">
      <img src={icon} alt="Rebost" class="h-20 w-20 animate-pulse rounded-[22%]" />
    </div>
  </div>
{:else if app.onboarding}
  <div class="flex h-full flex-col bg-navy-950">
    {#if mac}
      <div data-tauri-drag-region class="h-[46px] shrink-0"></div>
    {/if}
    <div class="min-h-0 flex-1">
      <Onboarding />
    </div>
  </div>
{:else}
  <div class="flex h-full bg-navy-950">
    <nav
      data-tauri-drag-region
      aria-label="Main"
      class="flex w-[76px] shrink-0 flex-col items-center border-r border-navy-950/40 bg-navy-950 pb-4 {mac
        ? 'pt-[46px]'
        : 'pt-4'}"
    >
      {#each nav as item (item.view)}
        {@const Icon = item.icon}
        <button
          type="button"
          aria-label={item.label}
          aria-current={app.view === item.view ? "page" : undefined}
          class="mb-1.5 flex w-[60px] flex-col items-center gap-1 rounded-xl py-2.5
            {app.view === item.view
            ? 'bg-white/12 text-amber-450'
            : 'text-navy-300 hover:bg-white/6 hover:text-white'}"
          onclick={() => {
            app.view = item.view;
            if (item.view === "shelves") app.openShelfId = null;
          }}
        >
          <Icon size={19} />
          <span class="text-[10px] font-medium">{item.label}</span>
        </button>
      {/each}
      <div class="flex-1"></div>
      {#if app.update}
        <button
          type="button"
          aria-label="Update available, version {app.update.version}"
          class="mb-1.5 flex w-[60px] flex-col items-center gap-1 rounded-xl py-2.5 text-amber-450
            hover:bg-white/6 hover:text-white"
          onclick={() => api.showUpdateWindow().catch(notifyInvokeError)}
        >
          <span class="relative">
            <ArrowUpCircle size={19} />
            <span
              class="absolute -top-0.5 -right-0.5 h-2 w-2 rounded-full bg-amber-450"
              aria-hidden="true"
            ></span>
          </span>
          <span class="text-[10px] font-medium">Update</span>
        </button>
      {/if}
      <button
        type="button"
        aria-label="Settings"
        aria-current={app.view === "settings" ? "page" : undefined}
        class="flex w-[60px] flex-col items-center gap-1 rounded-xl py-2.5
          {app.view === 'settings'
          ? 'bg-white/12 text-amber-450'
          : 'text-navy-300 hover:bg-white/6 hover:text-white'}"
        onclick={() => (app.view = "settings")}
      >
        <Settings size={19} />
        <span class="text-[10px] font-medium">Settings</span>
      </button>
    </nav>

    <main class="app-main-in flex min-w-0 flex-1 flex-col overflow-hidden bg-paper">
      {#if mac}
        <div data-tauri-drag-region class="h-3 shrink-0"></div>
      {/if}
      <div class="relative min-h-0 flex-1 overflow-hidden">
        {#key current}
          <div
            class="absolute inset-0 overflow-hidden"
            transition:fade={{ duration: allowViewFade ? motionMs(120) : 0 }}
          >
            {#if current === "chat"}
              <ChatView />
            {:else if current === "shelves"}
              {#await loadShelves()}
                <div class="h-full min-h-0" role="status">
                  <span class="sr-only">Loading Shelves</span>
                </div>
              {:then { default: ShelvesView }}
                <div class="h-full min-h-0 overflow-hidden"><ShelvesView /></div>
              {/await}
            {:else if current === "recipes"}
              {#await loadRecipes()}
                <div class="h-full" role="status">
                  <span class="sr-only">Loading Recipes</span>
                </div>
              {:then { default: RecipesView }}
                <div class="h-full overflow-y-auto"><RecipesView /></div>
              {/await}
            {:else}
              {#await loadSettings()}
                <div class="h-full" role="status">
                  <span class="sr-only">Loading Settings</span>
                </div>
              {:then { default: SettingsView }}
                <div class="h-full overflow-y-auto"><SettingsView /></div>
              {/await}
            {/if}
          </div>
        {/key}
      </div>
    </main>
  </div>
{/if}
