<script lang="ts">
  import { dialogPanel, overlay } from "$lib/motion";
  import { focusTrap } from "$lib/focus-trap";

  let {
    busy = false,
    onClose,
    onConfirm,
  }: {
    busy?: boolean;
    onClose: () => void;
    onConfirm: () => void | Promise<void>;
  } = $props();

  let typed = $state("");
  const matches = $derived(typed.trim() === "DELETE");
  const mismatch = $derived(typed.length > 0 && !matches);
  const hintId = "reset-confirm-hint";
  const errorId = "reset-confirm-error";

  function submit(event: Event) {
    event.preventDefault();
    if (!matches || busy) return;
    void onConfirm();
  }
</script>

<div
  class="fixed inset-0 z-40 flex items-center justify-center bg-navy-950/25 p-6"
  role="dialog"
  aria-modal="true"
  aria-labelledby="reset-dialog-title"
  aria-describedby="reset-dialog-body"
  aria-busy={busy}
  id="reset-workspace-dialog"
  tabindex="-1"
  use:focusTrap
  transition:overlay
  onclick={(e) => e.target === e.currentTarget && !busy && onClose()}
  onkeydown={(e) => e.key === "Escape" && !busy && onClose()}
>
  <form class="card w-full max-w-[420px] shadow-pop" onsubmit={submit} in:dialogPanel>
    <div class="px-5 pt-5 pb-4">
      <h2 id="reset-dialog-title" class="text-[16px] font-semibold text-ink">Reset Rebost?</h2>
      <p id="reset-dialog-body" class="mt-2 text-[13px] leading-relaxed text-ink-soft">
        This cannot be undone. Rebost will restart and open the first-run screen. Folders with your
        files stay on disk.
      </p>
      <label class="label mt-4 mb-1.5 block" for="reset-confirm">Type DELETE to confirm</label>
      <input
        id="reset-confirm"
        name="confirmation"
        class="input font-mono"
        type="text"
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        placeholder="DELETE"
        bind:value={typed}
        disabled={busy}
        aria-invalid={mismatch}
        aria-describedby={mismatch ? `${hintId} ${errorId}` : hintId}
        required
      />
      <p id={hintId} class="mt-1.5 text-[11.5px] text-ink-faint">Capitals are required.</p>
      {#if mismatch}
        <p id={errorId} class="mt-1 text-[12px] text-red-700" role="alert">Type DELETE exactly.</p>
      {/if}
    </div>
    <div class="flex items-center justify-end gap-2 border-t border-paper-line px-5 py-3">
      <button type="button" class="btn-outline" onclick={onClose} disabled={busy}>Cancel</button>
      <button
        type="submit"
        class="btn-danger"
        disabled={!matches || busy}
        aria-describedby={matches ? undefined : hintId}
      >
        {busy ? "Resetting…" : "Reset Rebost"}
      </button>
    </div>
  </form>
</div>
