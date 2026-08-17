<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { ArrowUpRight } from "@lucide/svelte";
  import { api, invokeError, type AboutInfo, type ExternalLink } from "$lib/api";
  import { isMac } from "$lib/platform";
  import icon from "../../assets/rebost-icon.png";

  let info = $state<AboutInfo | null>(null);
  const mac = isMac();

  onMount(() => {
    document.title = "About Rebost";
    api
      .aboutInfo()
      .then((value) => (info = value))
      .catch((error) => console.error(invokeError(error)));

    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        getCurrentWindow().close();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function openLink(event: MouseEvent, link: ExternalLink) {
    event.preventDefault();
    api.openExternal(link).catch((error) => console.error(invokeError(error)));
  }

  function sourceLabel(url: string): string {
    try {
      const parsed = new URL(url);
      return `${parsed.host}${parsed.pathname.replace(/\.git$/, "").replace(/\/$/, "")}`;
    } catch {
      return url;
    }
  }
</script>

{#snippet credit(label: string, href: string, link: ExternalLink, name: string)}
  <a
    {href}
    rel="noreferrer"
    class="group flex items-baseline justify-between gap-4 px-7 py-3 hover:bg-navy-950/[0.04]
      focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-amber-450"
    onclick={(event) => openLink(event, link)}
  >
    <span class="shrink-0 text-[11px] font-semibold tracking-wide text-ink-faint uppercase">
      {label}
    </span>
    <span class="flex min-w-0 items-center gap-1 text-right text-[13.5px] font-medium text-ink">
      <span class="truncate">{name}</span>
      <ArrowUpRight
        size={13}
        class="shrink-0 text-ink-faint transition-colors group-hover:text-amber-550"
      />
    </span>
  </a>
{/snippet}

<main class="flex h-full flex-col overflow-hidden bg-paper text-ink">
  {#if mac}
    <div data-tauri-drag-region class="h-[46px] shrink-0 bg-paper"></div>
  {/if}

  <header
    data-tauri-drag-region
    class="about-reveal flex flex-col items-center px-7 text-center {mac ? 'pt-1' : 'pt-6'}"
    style="animation-delay: 0ms"
  >
    <img
      src={icon}
      alt=""
      class="h-[72px] w-[72px] rounded-[22%] shadow-pop ring-[3px] ring-paper"
    />
    <h1 class="mt-3.5 text-[22px] font-semibold tracking-tight text-ink">Rebost</h1>
    <p class="mt-1 min-h-[1.2em] text-[11.5px] font-medium tracking-wide text-amber-550 uppercase">
      {#if info}Version {info.version}{/if}
    </p>
    <p class="mt-3 max-w-[320px] text-[13.5px] leading-relaxed text-ink-soft">
      Private AI that works with your files. What happens in your computer stays in your computer.
    </p>
  </header>

  <nav
    data-tauri-drag-region="false"
    aria-label="Credits"
    class="about-reveal mt-5"
    style="animation-delay: 80ms"
  >
    {#if info?.repositoryUrl}
      {@render credit("Source", info.repositoryUrl, "repository", sourceLabel(info.repositoryUrl))}
    {/if}
  </nav>
</main>

<style>
  @media (prefers-reduced-motion: no-preference) {
    .about-reveal {
      animation: about-in 320ms ease-out both;
    }
  }
  @keyframes about-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
