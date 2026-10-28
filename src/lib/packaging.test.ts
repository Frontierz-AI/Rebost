// The AUR and Flathub files repeat facts that live elsewhere in the repo.
// These checks keep them in step with a version bump or an engine bump.
import { describe, expect, it } from "vitest";
import packageJson from "../../package.json?raw";
import pin from "../../src-tauri/src/engine/pin.rs?raw";
import distribution from "../../src-tauri/src/distribution.rs?raw";
import manifest from "../../packaging/flatpak/ai.rebost.Rebost.yml?raw";
import metainfo from "../../packaging/flatpak/ai.rebost.Rebost.metainfo.xml?raw";
import desktop from "../../packaging/flatpak/ai.rebost.Rebost.desktop?raw";
import pkgbuild from "../../packaging/aur/rebost-bin/PKGBUILD?raw";

const version = JSON.parse(packageJson).version as string;

describe("Flatpak", () => {
  it("lists this version as the newest release in the metainfo", () => {
    const newest = metainfo.match(/<release version="([^"]+)"/)?.[1];
    expect(newest).toBe(version);
  });

  it("builds the llama.cpp release the app pins", () => {
    const build = pin.match(/ENGINE_BUILD: &str = "([^"]+)"/)?.[1];
    expect(build).toBeTruthy();
    expect(manifest).toContain(`tag: ${build}`);
    // The app finds the bundled builds by the upstream Linux file names.
    for (const accelerator of ["Vulkan", "CPU"]) {
      for (const arch of ["x86_64", "aarch64"]) {
        const row = new RegExp(
          `os: "linux",\\s*arch: "${arch}",[^}]*file_name: "([^"]+)",\\s*accelerator: "${accelerator}"`,
        );
        const fileName = pin.match(row)?.[1];
        expect(fileName, `${accelerator} ${arch}`).toBeTruthy();
        const suffix = arch === "x86_64" ? "x64" : "arm64";
        const written = fileName!.replace(new RegExp(`${suffix}\\.tar\\.gz$`), "$suffix.tar.gz");
        expect(manifest, fileName).toContain(written);
      }
    }
  });

  it("names the desktop entry, icon and metainfo after the app ID", () => {
    const id = manifest.match(/^id: (.+)$/m)?.[1];
    expect(id).toBe("ai.rebost.Rebost");
    expect(metainfo).toContain(`<id>${id}</id>`);
    expect(metainfo).toContain(`<launchable type="desktop-id">${id}.desktop</launchable>`);
    expect(desktop).toContain(`Icon=${id}\n`);
  });
});

describe("AUR", () => {
  it("writes the marker the app reads to leave updates to pacman", () => {
    const marker = distribution.match(/MARKER_FILE: &str = "([^"]+)"/)?.[1];
    expect(marker).toBe("package-manager");
    expect(pkgbuild).toContain(`"$pkgdir/usr/lib/Rebost/${marker}"`);
  });

  it("repackages the public .deb names", () => {
    expect(pkgbuild).toContain("Rebost-${_pkgver}-Linux-amd64.deb");
    expect(pkgbuild).toContain("Rebost-${_pkgver}-Linux-arm64.deb");
  });
});
