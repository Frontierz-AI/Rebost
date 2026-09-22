<script lang="ts">
  import { Image } from "@lucide/svelte";
</script>

<div
  class="relative flex aspect-4/3 w-full items-center justify-center overflow-hidden rounded-xl bg-paper-soft outline-1 -outline-offset-1 outline-ink/10"
  aria-hidden="true"
>
  <Image size={16} class="shrink-0 stroke-ink-faint" />
  <div
    class="shimmer pointer-events-none absolute inset-0 bg-linear-to-r from-transparent via-ink/5 to-transparent"
  ></div>
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
