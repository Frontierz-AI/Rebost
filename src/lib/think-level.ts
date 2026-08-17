import type { ThinkLevel } from "./api";

export const THINK_LEVELS = ["off", "light", "deep"] as const satisfies readonly ThinkLevel[];

export const THINK_LABELS: Record<ThinkLevel, string> = {
  off: "Off",
  light: "Light",
  deep: "Deep",
};

export function thinkLevelIndex(level: ThinkLevel): number {
  switch (level) {
    case "off":
      return 0;
    case "light":
      return 1;
    case "deep":
      return 2;
    default: {
      const _exhaustive: never = level;
      return _exhaustive;
    }
  }
}

export function thinkLevelFromIndex(index: number): ThinkLevel {
  const clamped = Math.max(0, Math.min(THINK_LEVELS.length - 1, Math.round(index)));
  return THINK_LEVELS[clamped] ?? "off";
}
