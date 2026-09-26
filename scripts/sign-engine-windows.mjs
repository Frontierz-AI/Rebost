#!/usr/bin/env node
// Windows release builds: bundle a llama.cpp zip whose llama-server.exe and
// DLLs carry Rebost's Authenticode signature. No-op unless one of these is set:
//   REBOST_SIGN_ENGINE=1          sign the staged zip here (x64, Azure env)
//   REBOST_SIGNED_ENGINE_DIR=dir  use a zip signed on another host (ARM64 builds)

import { spawnSync } from "node:child_process";
import { access, copyFile, readdir } from "node:fs/promises";
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
if (staged.length !== 1) {
  throw new Error(`expected one staged engine zip in ${STAGE}, found ${staged.length}`);
}
const archive = path.join(STAGE, staged[0]);

if (signedDir) {
  const signed = path.join(signedDir, staged[0]);
  try {
    await access(signed);
  } catch {
    console.warn(`no signed ${staged[0]} in ${signedDir}; bundling the unsigned engine`);
    process.exit(0);
  }
  await copyFile(signed, archive);
  console.log(`bundling signed ${staged[0]}`);
  process.exit(0);
}

// The script replaces the zip only after every file is signed. A failure
// leaves the upstream zip in place and must not cost the installer its own
// signature, so warn and carry on.
const script = path.join(ROOT, "scripts/sign-engine-windows.ps1");
const result = spawnSync("pwsh", ["-NoProfile", "-File", script, "-Archive", archive], {
  stdio: "inherit",
});
if (result.status !== 0) {
  console.warn(`::warning::engine signing failed; bundling the unsigned ${staged[0]}`);
}
