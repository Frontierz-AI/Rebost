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
pub const ENGINE_RELEASE: &str = "0.5.0";
/// GitHub tag that hosts the archives for [`ENGINE_RELEASE`].
pub const ENGINE_BUILD: &str = "b11146";

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
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-macos-arm64.tar.gz",
        sha256: "1ad3f9eff80edb9dbef4259ad564d1720612ef7eea48fa4afed0e54f5f3d5711",
        file_name: "llama-b11146-bin-macos-arm64.tar.gz",
        accelerator: "Metal",
    },
    EnginePin {
        os: "macos",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-macos-x64.tar.gz",
        sha256: "305f0e3a17d2c01eb205cd0a62128357f1ec3b55329cb084d94e5ec0115d7a3b",
        file_name: "llama-b11146-bin-macos-x64.tar.gz",
        accelerator: "Metal",
    },
    EnginePin {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-win-vulkan-x64.zip",
        sha256: "55a378aa095b466979d85075234f66d7655c7a7483222af0c006c0e55b4d7bd6",
        file_name: "llama-b11146-bin-win-vulkan-x64.zip",
        accelerator: "Vulkan",
    },
    EnginePin {
        os: "windows",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-win-cpu-arm64.zip",
        sha256: "1727d241f3bf6d27360e984e851cf013928fd655bf89f8628e70da027f377b7d",
        file_name: "llama-b11146-bin-win-cpu-arm64.zip",
        accelerator: "CPU",
    },
    EnginePin {
        os: "linux",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-ubuntu-vulkan-x64.tar.gz",
        sha256: "d3ce40fce7403cc93bcf5718fc46c6efb61ed9709f8e5d9f10c86bf0e30e8fb3",
        file_name: "llama-b11146-bin-ubuntu-vulkan-x64.tar.gz",
        accelerator: "Vulkan",
    },
    EnginePin {
        os: "linux",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-ubuntu-vulkan-arm64.tar.gz",
        sha256: "5dcebe3ecbcb43a1ed85e3284453f9edf54dcca833e1cb1f54b4022b753c1da5",
        file_name: "llama-b11146-bin-ubuntu-vulkan-arm64.tar.gz",
        accelerator: "Vulkan",
    },
];

/// Builds downloaded at warmup, never bundled. CUDA (~250 MB plus a ~370 MB
/// runtime zip) and Adreno OpenCL are faster GPU builds for matching hardware.
/// The Windows x64 CPU build (~18 MB) is the last resort when the bundled
/// Vulkan build will not start on a PC's graphics driver.
pub const ENGINE_OPTIONAL_PINS: &[EnginePin] = &[
    EnginePin {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-win-cpu-x64.zip",
        sha256: "14cf1303ca9ac3abd94816850532f9f9a69ac66fbaca3776fc6f9061c2fac1d1",
        file_name: "llama-b11146-bin-win-cpu-x64.zip",
        accelerator: "CPU",
    },
    EnginePin {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-win-cuda-12.4-x64.zip",
        sha256: "3c806a6ceccc3dae1c743ceb1a1fb2cce5b76f40bfbd4c6b7b8afb6ef45a5807",
        file_name: "llama-b11146-bin-win-cuda-12.4-x64.zip",
        accelerator: "CUDA",
    },
    EnginePin {
        os: "windows",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-win-opencl-adreno-arm64.zip",
        sha256: "e4f080b1951c80ed5a5f3a4d52bda31839ebdfb8285c39dbc1bd66e4cfc13806",
        file_name: "llama-b11146-bin-win-opencl-adreno-arm64.zip",
        accelerator: "OpenCL",
    },
];

pub const ENGINE_OPTIONAL_RUNTIMES: &[EngineRuntimePin] = &[EngineRuntimePin {
    os: "windows",
    arch: "x86_64",
    accelerator: "CUDA",
    url: "https://github.com/ggml-org/llama.cpp/releases/download/b11146/cudart-llama-bin-win-cuda-12.4-x64.zip",
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
        assert_eq!(ENGINE_RELEASE, "0.5.0");
        assert_eq!(ENGINE_BUILD, "b11146");
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
            "llama-b11146-bin-macos-arm64.tar.gz"
        );
        assert_eq!(
            pin_for_target_triple("x86_64-apple-darwin")
                .unwrap()
                .file_name,
            "llama-b11146-bin-macos-x64.tar.gz"
        );
        assert_eq!(
            pin_for_target_triple("x86_64-pc-windows-msvc")
                .unwrap()
                .file_name,
            "llama-b11146-bin-win-vulkan-x64.zip"
        );
        assert_eq!(
            pin_for_target_triple("aarch64-pc-windows-msvc")
                .unwrap()
                .file_name,
            "llama-b11146-bin-win-cpu-arm64.zip"
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
            format!("0.5.0-{}", native.accelerator.to_ascii_lowercase())
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
                "0.5.0-{}-{foreign_arch}",
                foreign.accelerator.to_ascii_lowercase()
            )
        );
    }

    #[test]
    fn only_a_gpu_bundle_has_a_cpu_fallback() {
        let fallback = cpu_fallback_pin_for("windows", "x86_64").unwrap();
        assert_eq!(fallback.accelerator, "CPU");
        assert_eq!(fallback.file_name, "llama-b11146-bin-win-cpu-x64.zip");
        assert!(cpu_fallback_pin_for("windows", "aarch64").is_none());
        assert!(cpu_fallback_pin_for("macos", "aarch64").is_none());
        assert!(cpu_fallback_pin_for("linux", "x86_64").is_none());
    }
}
