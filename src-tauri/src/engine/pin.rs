//! Pinned llama.cpp archives per OS and architecture.
//!
//! Contributors should not need a llama.cpp toolchain. Rebost follows
//! official `v*` releases, not nightlies. Those tags do not attach OS
//! archives; the binaries live on the nightly tag named in that release.
//! Bumping the engine means updating `ENGINE_RELEASE`, `ENGINE_BUILD`,
//! every row in `ENGINE_PINS`, and the optional GPU rows when those
//! archives move.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Official llama.cpp semver. Bump only when they cut a new `v*` release.
pub const ENGINE_RELEASE: &str = "0.7.0";
/// GitHub tag that hosts the archives for [`ENGINE_RELEASE`].
pub const ENGINE_BUILD: &str = "b11669";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnginePin {
    pub os: &'static str,
    pub arch: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub file_name: &'static str,
    /// What llama.cpp will use on this build: Metal, Vulkan, CUDA, OpenCL, or CPU.
    pub accelerator: &'static str,
}

/// CUDA redistributable unpacked beside `llama-server`. The CUDA llama.cpp zip
/// does not include `cudart` / `cublas`; without them the process exits before
/// `/health`. Not bundled — NVIDIA Windows only, first warmup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineRuntimePin {
    pub os: &'static str,
    pub arch: &'static str,
    pub accelerator: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub file_name: &'static str,
    /// File that must sit next to `llama-server` before this pin is ready.
    pub sidecar: &'static str,
}

/// Host-selected pin. CI HEAD-checks every `url`.
pub const ENGINE_PINS: &[EnginePin] = &[
    EnginePin {
        os: "macos",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-macos-arm64.tar.gz",
        sha256: "dbed55493d72bc1ba2babc8770d669c2289b5bc4ee9fb2a4dfc158d45b7d13fd",
        file_name: "llama-b11669-bin-macos-arm64.tar.gz",
        accelerator: "Metal",
    },
    EnginePin {
        os: "macos",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-macos-x64.tar.gz",
        sha256: "cab28238d78158109b48deb27b05d50e0dcef56275d2e76da9ca8281526c7532",
        file_name: "llama-b11669-bin-macos-x64.tar.gz",
        accelerator: "Metal",
    },
    EnginePin {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-win-vulkan-x64.zip",
        sha256: "2c07574e7bde71321cb785282b20cc821e951132149eeceeea32ddebdbf0598e",
        file_name: "llama-b11669-bin-win-vulkan-x64.zip",
        accelerator: "Vulkan",
    },
    EnginePin {
        os: "windows",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-win-cpu-arm64.zip",
        sha256: "9cff39ffed17edcd03a7d876073f74d7b48cd43a1e5eafdd59ee094185ad871b",
        file_name: "llama-b11669-bin-win-cpu-arm64.zip",
        accelerator: "CPU",
    },
    EnginePin {
        os: "linux",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-ubuntu-vulkan-x64.tar.gz",
        sha256: "e8709e5ba6c1b093dea7f3de57f4aba842f9a188fba6b465782d65d6fcfa4d9f",
        file_name: "llama-b11669-bin-ubuntu-vulkan-x64.tar.gz",
        accelerator: "Vulkan",
    },
    EnginePin {
        os: "linux",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-ubuntu-vulkan-arm64.tar.gz",
        sha256: "d283522e479c0d532e3a04d715dd852ffb2d95925caa6d41ef2ba53a65e46a51",
        file_name: "llama-b11669-bin-ubuntu-vulkan-arm64.tar.gz",
        accelerator: "Vulkan",
    },
];

/// Builds downloaded at warmup, never bundled. CUDA (~254 MiB plus a ~373 MiB
/// runtime zip) and Adreno OpenCL are faster GPU builds for matching hardware.
/// The Windows x64 CPU build (~19 MiB) is the last resort when the bundled
/// Vulkan build will not start on a PC's graphics driver.
pub const ENGINE_OPTIONAL_PINS: &[EnginePin] = &[
    EnginePin {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-win-cpu-x64.zip",
        sha256: "37b3efcf962847ca869745fd4ccf7f5e7e8dacfeff9fa3523b4ca8cdc66d632b",
        file_name: "llama-b11669-bin-win-cpu-x64.zip",
        accelerator: "CPU",
    },
    EnginePin {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-win-cuda-12.4-x64.zip",
        sha256: "c2da1f73ed176ae093ac5aceb978a7b167a16113f6d74c6789eeaee2135cb8dc",
        file_name: "llama-b11669-bin-win-cuda-12.4-x64.zip",
        accelerator: "CUDA",
    },
    EnginePin {
        os: "windows",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/llama-b11669-bin-win-opencl-adreno-arm64.zip",
        sha256: "3b10da78b5d7129bbafd302a95199233d2485536cc438eeb3d8ed1f904852ac0",
        file_name: "llama-b11669-bin-win-opencl-adreno-arm64.zip",
        accelerator: "OpenCL",
    },
];

pub const ENGINE_OPTIONAL_RUNTIMES: &[EngineRuntimePin] = &[EngineRuntimePin {
    os: "windows",
    arch: "x86_64",
    accelerator: "CUDA",
    url: "https://github.com/ggml-org/llama.cpp/releases/download/b11669/cudart-llama-bin-win-cuda-12.4-x64.zip",
        sha256: "8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6",
        file_name: "cudart-llama-bin-win-cuda-12.4-x64.zip",
    sidecar: "cudart64_12.dll",
}];

pub fn pin_for(os: &str, arch: &str) -> Result<&'static EnginePin> {
    ENGINE_PINS
        .iter()
        .find(|pin| pin.os == os && pin.arch == arch)
        .ok_or_else(|| anyhow!("Rebost has no llama.cpp build for {os}/{arch} yet"))
}

pub fn current_engine_pin() -> Result<&'static EnginePin> {
    pin_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Faster GPU archive for this machine, if one exists. Callers fall back to
/// [`current_engine_pin`] when download or spawn fails.
pub fn preferred_engine_pin() -> Result<&'static EnginePin> {
    if let Some(pin) = super::gpu::preferred_optional_pin() {
        return Ok(pin);
    }
    current_engine_pin()
}

pub fn optional_pin_for(os: &str, arch: &str, accelerator: &str) -> Option<&'static EnginePin> {
    ENGINE_OPTIONAL_PINS
        .iter()
        .find(|pin| pin.os == os && pin.arch == arch && pin.accelerator == accelerator)
}

/// CPU-only build tried after the bundled GPU build fails to start.
pub fn cpu_fallback_pin() -> Option<&'static EnginePin> {
    cpu_fallback_pin_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn cpu_fallback_pin_for(os: &str, arch: &str) -> Option<&'static EnginePin> {
    let bundled = pin_for(os, arch).ok()?;
    if bundled.accelerator == "CPU" {
        return None;
    }
    optional_pin_for(os, arch, "CPU")
}

pub fn runtime_for(pin: &EnginePin) -> Option<&'static EngineRuntimePin> {
    ENGINE_OPTIONAL_RUNTIMES.iter().find(|runtime| {
        runtime.os == pin.os && runtime.arch == pin.arch && runtime.accelerator == pin.accelerator
    })
}

pub fn extract_dir_name(pin: &EnginePin) -> String {
    let name = format!(
        "{}-{}",
        ENGINE_RELEASE,
        pin.accelerator.to_ascii_lowercase()
    );
    // The x64 Windows copy on an ARM PC runs the ARM64 CPU build. Keep it
    // apart from the x64 CPU build, which shares the accelerator name.
    if pin.arch == std::env::consts::ARCH {
        name
    } else {
        format!("{name}-{}", pin.arch)
    }
}

/// Map a Rust/Tauri target triple onto a pin (one installer per triple).
#[cfg(test)]
fn pin_for_target_triple(triple: &str) -> Result<&'static EnginePin> {
    let (os, arch) = match triple {
        "aarch64-apple-darwin" => ("macos", "aarch64"),
        "x86_64-apple-darwin" => ("macos", "x86_64"),
        "x86_64-pc-windows-msvc" => ("windows", "x86_64"),
        "aarch64-pc-windows-msvc" => ("windows", "aarch64"),
        "x86_64-unknown-linux-gnu" => ("linux", "x86_64"),
        "aarch64-unknown-linux-gnu" => ("linux", "aarch64"),
        other => {
            return Err(anyhow!(
                "Rebost has no llama.cpp build for target {other} yet"
            ));
        }
    };
    pin_for(os, arch)
}

pub fn find_bundled_engine_archive(
    roots: impl IntoIterator<Item = impl AsRef<Path>>,
    file_name: &str,
) -> Option<PathBuf> {
    roots.into_iter().find_map(|root| {
        let path = root.as_ref().join(file_name);
        path.is_file().then_some(path)
    })
}

pub fn llama_server_file_name() -> &'static str {
    if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    }
}

/// Discrete NVIDIA/AMD/Intel GPUs copy weights off mmap into RAM before
/// upload. Metal, Adreno, and CPU keep mmap (unified or page-cache).
pub fn no_mmap_for(pin: &EnginePin) -> bool {
    matches!(pin.accelerator, "Vulkan" | "CUDA")
}

pub fn is_llama_server_file_name(name: Option<&std::ffi::OsStr>) -> bool {
    matches!(
        name.and_then(|n| n.to_str()),
        Some("llama-server" | "llama-server.exe")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive_prefix() -> String {
        format!("https://github.com/ggml-org/llama.cpp/releases/download/{ENGINE_BUILD}/")
    }

    #[test]
    fn host_has_a_pin() {
        current_engine_pin().unwrap();
    }

    #[test]
    fn official_release_uses_named_nightly_archives() {
        assert_eq!(ENGINE_RELEASE, "0.7.0");
        assert_eq!(ENGINE_BUILD, "b11669");
    }

    #[test]
    fn pins_are_unique_and_well_formed() {
        let prefix = archive_prefix();
        let mut seen = std::collections::HashSet::new();
        for pin in ENGINE_PINS.iter().chain(ENGINE_OPTIONAL_PINS.iter()) {
            assert!(
                pin.url.starts_with(&prefix),
                "{} is not under the {ENGINE_BUILD} archives for {ENGINE_RELEASE}",
                pin.url
            );
            assert!(
                pin.url.ends_with(pin.file_name),
                "{} does not end with {}",
                pin.url,
                pin.file_name
            );
            assert_eq!(pin.sha256.len(), 64, "{}", pin.file_name);
            assert!(
                pin.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                "{}",
                pin.file_name
            );
            assert!(
                seen.insert((pin.os, pin.arch, pin.accelerator)),
                "duplicate pin for {}/{}/{}",
                pin.os,
                pin.arch,
                pin.accelerator
            );
        }
        let mut bundled = std::collections::HashSet::new();
        for pin in ENGINE_PINS {
            assert!(
                bundled.insert((pin.os, pin.arch)),
                "two bundled pins for {}/{}",
                pin.os,
                pin.arch
            );
        }
        for runtime in ENGINE_OPTIONAL_RUNTIMES {
            assert!(runtime.url.starts_with(&prefix), "{}", runtime.url);
            assert!(runtime.url.ends_with(runtime.file_name), "{}", runtime.url);
            assert_eq!(runtime.sha256.len(), 64, "{}", runtime.file_name);
            assert!(
                runtime.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                "{}",
                runtime.file_name
            );
            assert!(
                optional_pin_for(runtime.os, runtime.arch, runtime.accelerator).is_some(),
                "runtime {} has no matching optional pin",
                runtime.file_name
            );
        }
    }

    #[test]
    fn target_triples_match_desktop_pins() {
        assert_eq!(
            pin_for_target_triple("aarch64-apple-darwin")
                .unwrap()
                .file_name,
            "llama-b11669-bin-macos-arm64.tar.gz"
        );
        assert_eq!(
            pin_for_target_triple("x86_64-apple-darwin")
                .unwrap()
                .file_name,
            "llama-b11669-bin-macos-x64.tar.gz"
        );
        assert_eq!(
            pin_for_target_triple("x86_64-pc-windows-msvc")
                .unwrap()
                .file_name,
            "llama-b11669-bin-win-vulkan-x64.zip"
        );
        assert_eq!(
            pin_for_target_triple("aarch64-pc-windows-msvc")
                .unwrap()
                .file_name,
            "llama-b11669-bin-win-cpu-arm64.zip"
        );
        assert!(pin_for_target_triple("wasm32-unknown-unknown").is_err());
    }

    #[test]
    fn find_bundled_archive_picks_named_file() {
        let dir = tempfile::tempdir().unwrap();
        let name = "llama-fake.tar.gz";
        std::fs::write(dir.path().join(name), b"x").unwrap();
        let found = find_bundled_engine_archive([dir.path()], name).unwrap();
        assert_eq!(found.file_name().unwrap(), name);
        assert!(find_bundled_engine_archive([dir.path()], "missing.bin").is_none());
    }

    #[test]
    fn no_mmap_is_discrete_gpu_only() {
        let vulkan = pin_for("windows", "x86_64").unwrap();
        let cuda = optional_pin_for("windows", "x86_64", "CUDA").unwrap();
        let metal = pin_for("macos", "aarch64").unwrap();
        let cpu = pin_for("windows", "aarch64").unwrap();
        let opencl = optional_pin_for("windows", "aarch64", "OpenCL").unwrap();
        assert!(no_mmap_for(vulkan));
        assert!(no_mmap_for(cuda));
        assert!(!no_mmap_for(metal));
        assert!(!no_mmap_for(cpu));
        assert!(!no_mmap_for(opencl));
    }

    #[test]
    fn cuda_pin_has_a_runtime_sidecar() {
        let cuda = optional_pin_for("windows", "x86_64", "CUDA").unwrap();
        let runtime = runtime_for(cuda).unwrap();
        assert_eq!(runtime.sidecar, "cudart64_12.dll");
        assert_eq!(runtime.sha256.len(), 64);
        assert!(runtime.file_name.contains("cudart"));
        assert!(runtime_for(pin_for("windows", "x86_64").unwrap()).is_none());
        assert!(runtime_for(optional_pin_for("windows", "aarch64", "OpenCL").unwrap()).is_none());
    }

    #[test]
    fn extract_dir_is_release_plus_accelerator() {
        let native = pin_for(std::env::consts::OS, std::env::consts::ARCH).unwrap();
        assert_eq!(
            extract_dir_name(native),
            format!("0.7.0-{}", native.accelerator.to_ascii_lowercase())
        );
        let foreign_arch = if std::env::consts::ARCH == "aarch64" {
            "x86_64"
        } else {
            "aarch64"
        };
        let foreign = pin_for("windows", foreign_arch).unwrap();
        assert_eq!(
            extract_dir_name(foreign),
            format!(
                "0.7.0-{}-{foreign_arch}",
                foreign.accelerator.to_ascii_lowercase()
            )
        );
    }

    #[test]
    fn only_a_gpu_bundle_has_a_cpu_fallback() {
        let fallback = cpu_fallback_pin_for("windows", "x86_64").unwrap();
        assert_eq!(fallback.accelerator, "CPU");
        assert_eq!(fallback.file_name, "llama-b11669-bin-win-cpu-x64.zip");
        assert!(cpu_fallback_pin_for("windows", "aarch64").is_none());
        assert!(cpu_fallback_pin_for("macos", "aarch64").is_none());
        assert!(cpu_fallback_pin_for("linux", "x86_64").is_none());
    }
}
