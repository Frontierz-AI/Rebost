<script lang="ts">
  import { api, invokeError } from "$lib/api";
  import { notifyInvokeError } from "$lib/stores.svelte";
  import { Copy, ShieldCheck, Check } from "@lucide/svelte";

  let { text, subtle = false }: { text: string; subtle?: boolean } = $props();

  let hasPii = $state(false);
  let copied = $state<"plain" | "redacted" | null>(null);

  $effect(() => {
    const current = text;
    if (!current.trim()) {
      hasPii = false;
      return;
    }
    let alive = true;
    api
      .textHasPii(current)
      .then((v) => {
        if (alive) hasPii = v;
      })
      .catch((error) => {
        if (alive) console.error(invokeError(error));
      });
    return () => {
      alive = false;
    };
  });

  async function copyPlain() {
    try {
      await navigator.clipboard.writeText(text);
      copied = "plain";
      setTimeout(() => (copied = null), 1400);
    } catch (error) {
      notifyInvokeError(error);
    }
  }

  async function copyRedacted() {
    try {
      const redacted = await api.redactText(text);
      await navigator.clipboard.writeText(redacted);
      copied = "redacted";
      setTimeout(() => (copied = null), 1400);
    } catch (error) {
      notifyInvokeError(error);
    }
  }
</script>

<div class="flex items-center gap-1 {subtle ? 'text-ink-faint' : ''}">
  <button
    type="button"
    class="btn-ghost !px-2 !py-1 !text-[11.5px]"
    onclick={copyPlain}
    title="Copy"
  >
    {#if copied === "plain"}<Check
        size={13}
        class="text-emerald-600 dark:text-emerald-400"
      />{:else}<Copy size={13} />{/if}
    Copy
  </button>
  {#if hasPii}
    <button
      type="button"
      class="btn-ghost !px-2 !py-1 !text-[11.5px]"
      onclick={copyRedacted}
      title="Replaces recognized identifiers before copying"
    >
      {#if copied === "redacted"}<Check
          size={13}
          class="text-emerald-600 dark:text-emerald-400"
        />{:else}<ShieldCheck size={13} />{/if}
      Copy without personal information
    </button>
  {/if}
</div>
