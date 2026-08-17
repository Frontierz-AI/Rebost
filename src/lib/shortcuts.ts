import type { MenuAction } from "./api";
import { isMac } from "./platform";

export function isModKey(event: KeyboardEvent): boolean {
  return isMac() ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

export function shortcutAction(event: KeyboardEvent): MenuAction | null {
  if (!isModKey(event) || event.altKey || event.shiftKey || event.repeat) return null;
  switch (event.key) {
    case "n":
    case "N":
      return "new-conversation";
    case "1":
      return "view-chat";
    case "2":
      return "view-shelves";
    case "3":
      return "view-recipes";
    case ",":
      return "view-settings";
    default:
      return null;
  }
}

export function parseMenuAction(action: string): MenuAction | null {
  switch (action) {
    case "new-conversation":
    case "view-chat":
    case "view-shelves":
    case "view-recipes":
    case "view-settings":
      return action;
    default:
      return null;
  }
}
