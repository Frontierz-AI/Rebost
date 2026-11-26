<script lang="ts">
  import { formatWhen, type ShelfView } from "$lib/api";
  import { t } from "$lib/i18n.svelte";
  import { threadShelfSubtitle } from "$lib/shelf-label";
  import { moreMatches, type SearchGroups, type SearchRow } from "$lib/thread-search";
  import ConversationFace from "./ConversationFace.svelte";

  let {
    groups,
    activeIndex,
    activeThreadId,
    listboxId,
    shelves,
    onOpen,
  }: {
    groups: SearchGroups;
    /** Highlighted option across both groups; -1 for none. */
    activeIndex: number;
    activeThreadId: string | null;
    listboxId: string;
    shelves: ShelfView[];
    onOpen: (id: string) => void;
  } = $props();

  $effect(() => {
    if (activeIndex < 0) return;
    document.getElementById(`${listboxId}-${activeIndex}`)?.scrollIntoView({ block: "nearest" });
  });

  function moreLabel(row: SearchRow): string | null {
    const more = moreMatches(row.hit);
    if (!more) return null;
    if (more.atLeast) return t("chat.manyMoreMatchesAtLeast", { count: more.count });
    return more.count === 1
      ? t("chat.oneMoreMatch")
      : t("chat.manyMoreMatches", { count: more.count });
  }
</script>

{#snippet option(row: SearchRow, index: number)}
  {@const hit = row.hit}
  {@const more = moreLabel(row)}
  <!-- Focus stays in the search field (aria-activedescendant); Enter there opens the option. -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    id="{listboxId}-{index}"
    role="option"
    tabindex="-1"
    aria-selected={index === activeIndex}
    class="mb-0.5 flex min-h-[52px] items-start gap-2 rounded-lg px-2 py-2
      {index === activeIndex
      ? 'bg-paper-soft ring-1 ring-navy-500/40 ring-inset'
      : activeThreadId === row.thread.id
        ? 'bg-paper-soft/70'
        : 'hover:bg-paper-soft/60'}"
    onmousedown={(event) => event.preventDefault()}
    onclick={() => onOpen(row.thread.id)}
  >
    <span class="flex shrink-0 items-center pt-0.5">
      <ConversationFace avatarId={row.thread.avatarId} />
    </span>
    <span class="min-w-0 flex-1">
      <span class="block truncate text-[12.5px] font-medium text-ink">{row.thread.title}</span>
      {#if row.thread.shelfId}
        {@const subtitle = threadShelfSubtitle(row.thread, shelves)}
        {#if subtitle}<span class="block truncate text-[11px] text-ink-faint">{subtitle}</span>{/if}
      {/if}
      {#if hit && hit.snippet.length > 0}
        <span class="mt-0.5 line-clamp-2 text-[11.5px] leading-snug break-words text-ink-soft">
          {#each hit.snippet as segment, i (i)}{#if segment.mark}<mark
                class="rounded-[3px] bg-navy-500/25 px-px text-ink"
              >
                {segment.text}
              </mark>{:else}{segment.text}{/if}{/each}
        </span>
        <span class="mt-0.5 block truncate text-[10.5px] text-ink-faint">
          {hit.role === "user" ? t("chat.searchRoleYou") : t("appName")} · {formatWhen(
            hit.createdAt,
          )}{#if more}
            · {more}{/if}
        </span>
      {/if}
    </span>
  </div>
{/snippet}

<div id={listboxId} role="listbox" aria-label={t("chat.searchResults")}>
  {#if groups.titles.length > 0}
    <div role="group" aria-labelledby="{listboxId}-titles">
      <div
        id="{listboxId}-titles"
        role="presentation"
        class="px-2.5 pt-1 pb-1 text-[11px] font-semibold text-ink-faint"
      >
        {t("chat.searchTitles")}
      </div>
      {#each groups.titles as row, index (row.thread.id)}
        {@render option(row, index)}
      {/each}
    </div>
  {/if}
  {#if groups.messages.length > 0}
    <div role="group" aria-labelledby="{listboxId}-messages">
      <div
        id="{listboxId}-messages"
        role="presentation"
        class="px-2.5 pt-2 pb-1 text-[11px] font-semibold text-ink-faint"
      >
        {t("chat.searchMessages")}
      </div>
      {#each groups.messages as row, index (row.thread.id)}
        {@render option(row, groups.titles.length + index)}
      {/each}
    </div>
  {/if}
</div>
