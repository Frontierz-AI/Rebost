import { confirm } from "@tauri-apps/plugin-dialog";

/** OS warning dialog (Ok / Cancel). Falls back to `window.confirm` outside Tauri. */
export async function confirmDanger(message: string, okLabel: string): Promise<boolean> {
  try {
    return await confirm(message, {
      title: "Rebost",
      kind: "warning",
      okLabel,
      cancelLabel: "Cancel",
    });
  } catch {
    return window.confirm(message);
  }
}
