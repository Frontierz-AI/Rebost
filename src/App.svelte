<script lang="ts">
  import { onMount } from "svelte";
  import { Toaster, toast } from "svelte-sonner";
  import { app, bootstrap, setNotifier } from "$lib/stores.svelte";
  import { api } from "$lib/api";
  import ChatView from "$lib/views/ChatView.svelte";
  import ShelvesView from "$lib/views/ShelvesView.svelte";
  import RecipesView from "$lib/views/RecipesView.svelte";
  import SettingsView from "$lib/views/SettingsView.svelte";
  import Onboarding from "$lib/views/Onboarding.svelte";
  import { MessageCircle, LibraryBig, ChefHat, Settings, ArrowUpCircle } from "@lucide/svelte";
  import icon from "./assets/rebost-icon.png";

  onMount(() => {
    setNotifier((message) => toast(message));
    bootstrap().catch((error) => {
      console.error(error);
      toast("Rebost couldn't finish starting. Try again.");
    });
  });

  const nav = [
    { view: "chat", label: "Chat", icon: MessageCircle },
    { view: "shelves", label: "Shelves", icon: LibraryBig },
    { view: "recipes", label: "Recipes", icon: ChefHat },
  ] as const;

  // macOS uses an overlay title bar: content runs full height and the
  // traffic lights float over the sidebar, so it needs top clearance and
  // draggable surfaces. Windows keeps its native title bar untouched.
  const isMac = navigator.userAgent.includes("Mac");
</script>

<Toaster position="bottom-right" richColors />

{#if !app.ready}
  <div class="flex h-full flex-col bg-navy-950">
    {#if isMac}
      <div data-tauri-drag-region class="h-[46px] shrink-0"></div>
    {/if}
    <div data-tauri-drag-region class="flex flex-1 items-center justify-center">
      <img src={icon} alt="Rebost" class="h-20 w-20 animate-pulse rounded-[22%]" />
    </div>
  </div>
{:else if app.onboarding}
  <div class="flex h-full flex-col bg-navy-950">
    {#if isMac}
      <div data-tauri-drag-region class="h-[46px] shrink-0"></div>
    {/if}
    <div class="min-h-0 flex-1">
      <Onboarding />
    </div>
  </div>
{:else}
  <div class="flex h-full">
    <!-- ── Sidebar ──────────────────────────────────────────────────── -->
    <nav
      data-tauri-drag-region
      aria-label="Main"
      class="flex w-[76px] shrink-0 flex-col items-center border-r border-navy-950/40 bg-navy-950 pb-4 {isMac
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
          onclick={() => api.showUpdateWindow().catch(() => {})}
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

    <!-- ── Active view ──────────────────────────────────────────────── -->
    <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
      {#if isMac}
        <!-- Slim grab strip standing in for the hidden title bar. -->
        <div data-tauri-drag-region class="h-3 shrink-0"></div>
      {/if}
      <div class="min-h-0 flex-1 overflow-hidden">
        {#if app.view === "chat"}
          <ChatView />
        {:else if app.view === "shelves"}
          <div class="h-full min-h-0 overflow-hidden"><ShelvesView /></div>
        {:else if app.view === "recipes"}
          <div class="h-full overflow-y-auto"><RecipesView /></div>
        {:else}
          <div class="h-full overflow-y-auto"><SettingsView /></div>
        {/if}
      </div>
    </main>
  </div>
{/if}
