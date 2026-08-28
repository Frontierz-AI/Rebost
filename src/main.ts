import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./app.css";
import App from "./App.svelte";
import AboutWindow from "./lib/views/AboutWindow.svelte";
import UpdateWindow from "./lib/views/UpdateWindow.svelte";
import { applyColorSchemeFromSystem, watchWindowTheme } from "./lib/appearance";
import { restoreTextSize } from "./lib/text-size";
import { suppressWebviewBeep } from "./lib/keys";
import { installNativeContextMenus } from "./lib/native-menu";
import { osFamily } from "./lib/platform";

document.documentElement.dataset.os = osFamily();
applyColorSchemeFromSystem();
void watchWindowTheme();
window.addEventListener("keydown", suppressWebviewBeep);
installNativeContextMenus();

type ExtraWindow = "about" | "update";

function extraWindow(): ExtraWindow | null {
  const param = new URLSearchParams(window.location.search).get("window");
  if (param === "about" || param === "update") {
    return param;
  }
  try {
    const label = getCurrentWindow().label;
    if (label === "about" || label === "update") {
      return label;
    }
  } catch {
    return null;
  }
  return null;
}

const target = document.getElementById("app")!;
const kind = extraWindow();
let app;
switch (kind) {
  case "about":
    app = mount(AboutWindow, { target });
    break;
  case "update":
    app = mount(UpdateWindow, { target });
    break;
  case null:
    restoreTextSize();
    app = mount(App, { target });
    break;
  default: {
    const _exhaustive: never = kind;
    app = mount(App, { target });
    void _exhaustive;
  }
}

export default app;
