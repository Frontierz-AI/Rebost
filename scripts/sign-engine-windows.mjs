#!/usr/bin/env node
// Windows release builds: bundle a llama.cpp zip whose llama-server.exe and
// DLLs carry Rebost's Authenticode signature. No-op unless one of these is set:
//   REBOST_SIGN_ENGINE=1          sign the staged zip here (x64, Azure env)
//   REBOST_SIGNED_ENGINE_DIR=dir  use a zip signed on another host (ARM64 builds)

import { spawnSync } from "node:child_process";
import { copyFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const signHere = process.env.REBOST_SIGN_ENGINE === "1";
const signedDir = process.env.REBOST_SIGNED_ENGINE_DIR;
if (process.platform !== "win32" || (!signHere && !signedDir)) {
  process.exit(0);
}

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const STAGE = path.join(ROOT, "src-tauri/resources/engine");
const staged = (await readdir(STAGE)).filter((name) => name.endsWith(".zip"));
if (!staged.length) throw new Error("Windows engine archives are missing");
for (const name of staged) {
  const archive = path.join(STAGE, name);
  if (signedDir) {
    await copyFile(path.join(signedDir, name), archive);
  } else {
    const script = path.join(ROOT, "scripts/sign-engine-windows.ps1");
    const result = spawnSync("pwsh", ["-NoProfile", "-File", script, "-Archive", archive], {
      stdio: "inherit",
    });
    if (result.status !== 0) throw new Error(`Signing failed for ${name}`);
  }
}
