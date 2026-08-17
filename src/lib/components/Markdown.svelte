<script lang="ts">
  import { renderMarkdown } from "$lib/markdown";
  import type { SourcePassage } from "$lib/api";

  let {
    text,
    sources = [],
    streaming = false,
    onCite,
  }: {
    text: string;
    sources?: SourcePassage[];
    streaming?: boolean;
    onCite?: (source: SourcePassage) => void;
  } = $props();

  const html = $derived(renderMarkdown(text, sources));

  function handleClick(event: MouseEvent) {
    const target = (event.target as HTMLElement).closest?.(".cite-chip") as HTMLElement | null;
    if (target?.dataset.sid && onCite) {
      const source = sources.find((s) => s.sid === target.dataset.sid);
      if (source) onCite(source);
    }
  }

  function citeClicks(node: HTMLElement) {
    node.addEventListener("click", handleClick);
    return {
      destroy() {
        node.removeEventListener("click", handleClick);
      },
    };
  }
</script>

<div class="md-body {streaming ? 'stream-caret' : ''}" use:citeClicks>
  {@html html}
</div>
