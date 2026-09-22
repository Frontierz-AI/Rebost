<script lang="ts">
  import { Image } from "@lucide/svelte";

  let { framed = true }: { framed?: boolean } = $props();
</script>

<div
  class="relative flex aspect-4/3 w-full items-center justify-center rounded-xl bg-paper-soft {framed
    ? 'border border-ink/10'
    : ''}"
  aria-hidden="true"
>
  <div
    class="absolute inset-0 flex items-center justify-center overflow-hidden rounded-[calc(0.75rem-1px)]"
  >
    <Image size={16} class="shrink-0 stroke-ink-faint" />
    <div
      class="shimmer pointer-events-none absolute inset-0 bg-linear-to-r from-transparent via-ink/5 to-transparent"
    ></div>
  </div>
</div>

<style>
  @media (prefers-reduced-motion: no-preference) {
    .shimmer {
      animation: image-shimmer 1.6s ease-in-out infinite;
    }
  }

  @keyframes image-shimmer {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(100%);
    }
  }
</style>
