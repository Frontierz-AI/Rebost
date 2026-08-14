<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, events, formatTransferBytes, type AppUpdate, type UpdateProgress } from "$lib/api";
  import { focusTrap } from "$lib/focus-trap";
  import icon from "../../assets/rebost-icon.png";

  let info = $state<AppUpdate | null>(null);
  let installing = $state(false);
  let error = $state<string | null>(null);
  let downloaded = $state(0);
  let contentLength = $state<number | null>(null);
  const isMac = navigator.userAgent.includes("Mac");

  const percent = $derived(
    contentLength && contentLength > 0
      ? Math.min(100, Math.round((downloaded / contentLength) * 100))
      : installing
        ? 25
        : 0,
  );

  onMount(() => {
    document.title = "Update Rebost";
    api
      .updateInfo()
      .then((value) => (info = value))
      .catch(() => {});

    const stop = events.updateProgress((event) => applyProgress(event));

    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape" && !installing) {
        getCurrentWindow().close();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      stop.then((unlisten) => unlisten());
    };
  });

  function applyProgress(event: UpdateProgress) {
    switch (event.event) {
      case "started":
        contentLength = event.data.contentLength ?? null;
        downloaded = 0;
        break;
      case "progress":
        downloaded += event.data.chunkLength;
        break;
      case "finished":
        if (contentLength) downloaded = contentLength;
        break;
      default: {
        const _exhaustive: never = event;
        return _exhaustive;
      }
    }
  }

  async function close() {
    await getCurrentWindow().close();
  }

  async function install() {
    if (!info || installing) return;
    installing = true;
    error = null;
    downloaded = 0;
    contentLength = null;
    try {
      await api.installUpdate();
    } catch {
      error = "The update couldn't be installed. Try again.";
      installing = false;
    }
  }
</script>

<main class="flex h-full flex-col overflow-hidden bg-paper text-ink" use:focusTrap>
  {#if isMac}
    <div data-tauri-drag-region class="h-[46px] shrink-0 bg-paper"></div>
  {/if}

  <header
    data-tauri-drag-region
    class="about-reveal flex flex-col items-center px-7 text-center {isMac ? 'pt-1' : 'pt-6'}"
  >
    <img
      src={icon}
      alt=""
      class="h-[72px] w-[72px] rounded-[22%] shadow-pop ring-[3px] ring-paper"
    />
    <h1 class="mt-3.5 text-[22px] font-semibold tracking-tight text-navy-950">Update Rebost</h1>
    {#if info}
      <p class="mt-1 text-[11.5px] font-medium tracking-wide text-amber-550 uppercase">
        Version {info.version} is available
      </p>
      <p class="mt-3 max-w-[320px] text-[13.5px] leading-relaxed text-ink-soft">
        You are on {info.currentVersion}. Updating replaces this app and restarts Rebost.
      </p>
    {:else}
      <p class="mt-3 max-w-[320px] text-[13.5px] leading-relaxed text-ink-soft">
        You are on the latest version.
      </p>
    {/if}
  </header>

  {#if info?.notes}
    <p
      class="about-reveal mx-7 mt-4 max-h-20 overflow-y-auto text-center text-[12.5px] leading-relaxed text-ink-soft"
    >
      {info.notes}
    </p>
  {/if}

  {#if installing}
    <div class="about-reveal mx-7 mt-5">
      <div class="flex items-center justify-between text-[13px]">
        <p class="font-medium text-ink">Downloading the update…</p>
        <p class="tabular-nums text-ink-soft">
          {formatTransferBytes(downloaded)}{contentLength
            ? ` / ${formatTransferBytes(contentLength)}`
            : ""}
        </p>
      </div>
      <div
        class="mt-2 h-1.5 overflow-hidden rounded-full bg-navy-100"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={contentLength ? percent : undefined}
        aria-label="Downloading the update"
      >
        <div
          class="h-full rounded-full bg-amber-450 w-(--progress)"
          style="--progress: {percent}%"
        ></div>
      </div>
    </div>
  {/if}

  {#if error}
    <p class="mx-7 mt-3 text-center text-[12.5px] text-red-700" role="alert">{error}</p>
  {/if}

  <div class="about-reveal mt-auto flex justify-end gap-2 px-7 py-6">
    <button type="button" class="btn-ghost" disabled={installing} onclick={close}>Later</button>
    {#if info}
      <button type="button" class="btn-primary" disabled={installing} onclick={install}>
        {installing ? "Updating…" : "Update"}
      </button>
    {/if}
  </div>
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
