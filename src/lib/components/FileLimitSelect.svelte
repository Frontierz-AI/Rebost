<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { setShelfFileLimit } from "$lib/stores.svelte";
  import { FILE_LIMIT_CHOICES, formatCount } from "$lib/shelf-cap";

  let {
    shelfId,
    value,
    files,
  }: {
    shelfId: string;
    value: number;
    /** Files on the Shelf now. */
    files: number;
  } = $props();

  const selectId = $derived(`file-limit-${shelfId}`);
  const hintId = $derived(`file-limit-hint-${shelfId}`);
  const choices = $derived(
    (FILE_LIMIT_CHOICES as readonly number[]).includes(value)
      ? FILE_LIMIT_CHOICES
      : [...FILE_LIMIT_CHOICES, value].sort((a, b) => a - b),
  );
  const over = $derived(files > value);

  function onChange(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLSelectElement)) return;
    void setShelfFileLimit(shelfId, Number(target.value));
  }
</script>

<div class="flex w-full min-w-[200px] flex-col gap-2">
  <label class="text-[12px] font-medium text-ink" for={selectId}>
    {t("shelves.fileLimit")}
  </label>
  <span class="select-wrap w-fit">
    <select
      id={selectId}
      name={selectId}
      class="input select !w-auto !py-1.5 !text-[12.5px] tabular-nums"
      aria-describedby={hintId}
      {value}
      onchange={onChange}
    >
      {#each choices as choice (choice)}
        <option value={choice}
          >{t("shelves.fileLimitChoice", { count: formatCount(choice) })}</option
        >
      {/each}
    </select>
    <svg
      viewBox="0 0 8 5"
      width="8"
      height="5"
      fill="none"
      class="pointer-events-none col-start-2 row-start-1 place-self-center text-ink-faint"
      aria-hidden="true"
    >
      <path d="M.5.5 4 4 7.5.5" stroke="currentcolor" stroke-linecap="round" />
    </svg>
  </span>
  <p id={hintId} class="text-[11.5px] text-ink-faint">
    {over ? t("shelves.fileLimitOver", { count: formatCount(files) }) : t("shelves.fileLimitHint")}
  </p>
</div>
