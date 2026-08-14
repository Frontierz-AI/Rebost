//! Pinned llama.cpp archives per OS and architecture.
//!
//! Contributors should not need a llama.cpp toolchain. Rebost downloads one
//! of these GitHub release artifacts, SHA-256-checks it, and runs
//! `llama-server` on loopback. Bumping the engine means updating `ENGINE_BUILD`
//! and every row in `ENGINE_PINS`.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// llama.cpp release tag. Keep in sync with the URLs below.
pub const ENGINE_BUILD: &str = "b10418";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnginePin {
    pub os: &'static str,
    pub arch: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub file_name: &'static str,
    /// What llama.cpp will use on this build: Metal, Vulkan, or CPU.
    pub accelerator: &'static str,
}

/// Host-selected pin. CI HEAD-checks every `url`.
pub const ENGINE_PINS: &[EnginePin] = &[
    EnginePin {
        os: "macos",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b10418/llama-b10418-bin-macos-arm64.tar.gz",
        sha256: "9f3f3a86c05dd068e507acbd0887eff0fb81e5da7d8368d3d6a2fd1e4290ff9f",
        file_name: "llama-b10418-bin-macos-arm64.tar.gz",
        accelerator: "Metal",
    },
    EnginePin {
        os: "macos",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b10418/llama-b10418-bin-macos-x64.tar.gz",
        sha256: "e80dc66e722db855ff96b08f0a4d9a812cd13ea073fc94f32c58e53452816324",
        file_name: "llama-b10418-bin-macos-x64.tar.gz",
        accelerator: "Metal",
    },
    EnginePin {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b10418/llama-b10418-bin-win-vulkan-x64.zip",
        sha256: "16137e088046681f3d7653538abeadd422031a205b34b14028cb343aa3c38add",
        file_name: "llama-b10418-bin-win-vulkan-x64.zip",
        accelerator: "Vulkan",
    },
    EnginePin {
        os: "windows",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b10418/llama-b10418-bin-win-cpu-arm64.zip",
        sha256: "9852af3469f129f839395b522588e83a1644862439570f1eff36f7ceac100974",
        file_name: "llama-b10418-bin-win-cpu-arm64.zip",
        accelerator: "CPU",
    },
    EnginePin {
        os: "linux",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b10418/llama-b10418-bin-ubuntu-vulkan-x64.tar.gz",
        sha256: "85099c4309a1aafaad8ae8520a7f091b9343f47c44a47f2d81965b9e6985ccee",
        file_name: "llama-b10418-bin-ubuntu-vulkan-x64.tar.gz",
        accelerator: "Vulkan",
    },
    EnginePin {
        os: "linux",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b10418/llama-b10418-bin-ubuntu-vulkan-arm64.tar.gz",
        sha256: "5d9b7d9180d643619851d525a182600b07b12049a2c489a4a2c6db42bdf8a008",
        file_name: "llama-b10418-bin-ubuntu-vulkan-arm64.tar.gz",
        accelerator: "Vulkan",
    },
];

pub fn pin_for(os: &str, arch: &str) -> Result<&'static EnginePin> {
    ENGINE_PINS
        .iter()
        .find(|pin| pin.os == os && pin.arch == arch)
        .ok_or_else(|| anyhow!("Rebost has no llama.cpp build for {os}/{arch} yet"))
}

pub fn current_engine_pin() -> Result<&'static EnginePin> {
    pin_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Map a Rust/Tauri target triple onto a pin (one installer per triple).
pub fn pin_for_target_triple(triple: &str) -> Result<&'static EnginePin> {
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

pub fn is_llama_server_file_name(name: Option<&std::ffi::OsStr>) -> bool {
    matches!(
        name.and_then(|n| n.to_str()),
        Some("llama-server" | "llama-server.exe")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_has_a_pin() {
        current_engine_pin().unwrap();
    }

    #[test]
    fn pins_are_unique_and_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for pin in ENGINE_PINS {
            assert!(
                pin.url
                    .starts_with("https://github.com/ggml-org/llama.cpp/releases/download/b10418/"),
                "{} is not under the b10418 release",
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
                seen.insert((pin.os, pin.arch)),
                "duplicate pin for {}/{}",
                pin.os,
                pin.arch
            );
        }
    }

    #[test]
    fn target_triples_match_desktop_pins() {
        assert_eq!(
            pin_for_target_triple("aarch64-apple-darwin")
                .unwrap()
                .file_name,
            "llama-b10418-bin-macos-arm64.tar.gz"
        );
        assert_eq!(
            pin_for_target_triple("x86_64-apple-darwin")
                .unwrap()
                .file_name,
            "llama-b10418-bin-macos-x64.tar.gz"
        );
        assert_eq!(
            pin_for_target_triple("x86_64-pc-windows-msvc")
                .unwrap()
                .file_name,
            "llama-b10418-bin-win-vulkan-x64.zip"
        );
        assert_eq!(
            pin_for_target_triple("aarch64-pc-windows-msvc")
                .unwrap()
                .file_name,
            "llama-b10418-bin-win-cpu-arm64.zip"
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
}
