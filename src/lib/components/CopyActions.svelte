<script lang="ts">
  import { api } from "$lib/api";
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
    api.textHasPii(current).then((v) => {
      if (alive) hasPii = v;
    });
    return () => {
      alive = false;
    };
  });

  async function copyPlain() {
    await navigator.clipboard.writeText(text);
    copied = "plain";
    setTimeout(() => (copied = null), 1400);
  }

  async function copyRedacted() {
    const redacted = await api.redactText(text);
    await navigator.clipboard.writeText(redacted);
    copied = "redacted";
    setTimeout(() => (copied = null), 1400);
  }
</script>

<div class="flex items-center gap-1 {subtle ? 'text-ink-faint' : ''}">
  <button
    type="button"
    class="btn-ghost !px-2 !py-1 !text-[11.5px]"
    onclick={copyPlain}
    title="Copy"
  >
    {#if copied === "plain"}<Check size={13} class="text-emerald-600" />{:else}<Copy
        size={13}
      />{/if}
    Copy
  </button>
  {#if hasPii}
    <button
      type="button"
      class="btn-ghost !px-2 !py-1 !text-[11.5px]"
      onclick={copyRedacted}
      title="Replaces recognized identifiers before copying"
    >
      {#if copied === "redacted"}<Check size={13} class="text-emerald-600" />{:else}<ShieldCheck
          size={13}
        />{/if}
      Copy without personal information
    </button>
  {/if}
</div>
