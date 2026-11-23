import { platform, version } from "@tauri-apps/plugin-os";

export type OsFamily = "macos" | "windows" | "other";

function fromUserAgent(): OsFamily {
  const ua = globalThis.navigator?.userAgent ?? "";
  if (ua.includes("Windows")) return "windows";
  if (ua.includes("Mac")) return "macos";
  return "other";
}

/** macOS or Windows when the OS plugin is available; user agent otherwise. */
export function osFamily(): OsFamily {
  try {
    const name = platform();
    if (name === "macos") return "macos";
    if (name === "windows") return "windows";
    return "other";
  } catch {
    return fromUserAgent();
  }
}

export function isMac(): boolean {
  return osFamily() === "macos";
}

export function isWindows(): boolean {
  return osFamily() === "windows";
}

/** Windows 11 is NT 10.0 from build 22000; Mica needs it. */
export function supportsMica(osVersion: string): boolean {
  const build = Number(osVersion.split(".")[2]);
  return Number.isFinite(build) && build >= 22000;
}

/**
 * System material behind the main window's sidebar, or `null` when the window
 * is opaque. Matches `windowEffects` in tauri.macos.conf.json and
 * tauri.windows.conf.json (Windows 10 ignores Mica).
 */
export function shellMaterial(): "sidebar" | "mica" | null {
  const os = osFamily();
  switch (os) {
    case "macos":
      return "sidebar";
    case "windows":
      try {
        return supportsMica(version()) ? "mica" : null;
      } catch {
        return null;
      }
    case "other":
      return null;
    default: {
      const _exhaustive: never = os;
      return _exhaustive;
    }
  }
}
