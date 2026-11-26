<script lang="ts">
  import { api, type ApiAccess } from "$lib/api";
  import { t } from "$lib/i18n.svelte";
  import { app, logInvokeError, notifyInvokeError } from "$lib/stores.svelte";
  import { Check, Copy, Eye, EyeOff } from "@lucide/svelte";

  let access = $state<ApiAccess | null>(null);
  let showKey = $state(false);
  let copied = $state<"url" | "key" | null>(null);

  // The port is new each time the AI starts, so read it again on every state change.
  $effect(() => {
    void app.engine.state;
    let alive = true;
    api
      .engineApiAccess()
      .then((value) => {
        if (alive) access = value;
      })
      .catch((error) => logInvokeError(error, "API access"));
    return () => {
      alive = false;
    };
  });

  async function copy(which: "url" | "key", text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = which;
      setTimeout(() => {
        if (copied === which) copied = null;
      }, 1400);
    } catch (error) {
      notifyInvokeError(error);
    }
  }
</script>

{#snippet copyButton(which: "url" | "key", text: string)}
  <button
    type="button"
    class="btn-ghost shrink-0 !px-2 !py-1 !text-[11.5px]"
    onclick={() => copy(which, text)}
  >
    {#if copied === which}
      <Check size={12} aria-hidden="true" />
      {t("apiAccess.copied")}
    {:else}
      <Copy size={12} aria-hidden="true" />
      {t("copy.copy")}
    {/if}
  </button>
{/snippet}

<section class="card mb-6 px-4 py-5 min-[900px]:px-6" aria-labelledby="api-access-heading">
  <h2 id="api-access-heading" class="mb-1 text-[15px] font-semibold text-ink">
    {t("apiAccess.heading")}
  </h2>
  <p class="mb-4 text-[12.5px] leading-snug text-ink-soft">{t("apiAccess.help")}</p>

  {#if access}
    <dl class="grid gap-3 text-[13px]">
      <div>
        <dt class="mb-1 text-[11.5px] font-medium text-ink-soft">{t("apiAccess.address")}</dt>
        <dd class="flex min-w-0 items-center gap-2">
          {#if access.url}
            <code
              class="min-w-0 flex-1 truncate rounded-lg border border-paper-line bg-paper-soft px-3 py-1.5 font-mono text-[12px] text-ink"
            >
              {access.url}
            </code>
            {@render copyButton("url", access.url)}
          {:else}
            <p class="text-[12.5px] text-ink-faint">{t("apiAccess.notRunning")}</p>
          {/if}
        </dd>
      </div>
      <div>
        <dt class="mb-1 text-[11.5px] font-medium text-ink-soft">{t("apiAccess.key")}</dt>
        <dd class="flex min-w-0 items-center gap-2">
          <code
            class="min-w-0 flex-1 truncate rounded-lg border border-paper-line bg-paper-soft px-3 py-1.5 font-mono text-[12px] text-ink"
          >
            {showKey ? access.key : "•".repeat(24)}
          </code>
          <button
            type="button"
            class="btn-ghost shrink-0 !px-2 !py-1 !text-[11.5px]"
            aria-pressed={showKey}
            onclick={() => (showKey = !showKey)}
          >
            {#if showKey}
              <EyeOff size={12} aria-hidden="true" />
              {t("apiAccess.hide")}
            {:else}
              <Eye size={12} aria-hidden="true" />
              {t("apiAccess.show")}
            {/if}
          </button>
          {@render copyButton("key", access.key)}
        </dd>
      </div>
    </dl>
  {/if}
</section>
