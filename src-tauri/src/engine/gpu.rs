//! Pick a faster llama.cpp build when the GPU is actually there.
//!
//! The installer still ships the portable pin (Metal / Vulkan / CPU). CUDA
//! and Adreno OpenCL archives are downloaded on first warmup only.

use super::pin::{optional_pin_for, EnginePin};

/// Downloaded pin for this machine, if a better archive exists than the
/// bundled one and the hardware can load it.
pub fn preferred_optional_pin() -> Option<&'static EnginePin> {
    if runs_x64_on_arm() {
        // The x64 copy on an ARM PC runs the native ARM64 CPU build. Under
        // emulation the bundled Vulkan build still sends prompt batches to
        // the Adreno GPU, which hangs or writes garbage.
        return super::pin::pin_for("windows", "aarch64").ok();
    }
    let accel = detected_accelerator()?;
    optional_pin_for(std::env::consts::OS, std::env::consts::ARCH, accel)
}

fn detected_accelerator() -> Option<&'static str> {
    #[cfg(all(windows, any(target_arch = "x86_64", target_arch = "aarch64")))]
    if !runs_x64_on_arm() && nvidia_driver_present() {
        return Some("CUDA");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    if std::path::Path::new("/dev/nvidiactl").exists() {
        return Some("CUDA");
    }
    #[cfg(all(windows, target_arch = "aarch64"))]
    if adreno_present() {
        return Some("OpenCL");
    }
    None
}

/// Each pinned binary has one GPU backend. Cache its device probe for this session.
pub(crate) async fn device_listed(binary: &std::path::Path, backend: &'static str) -> Option<bool> {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    static SEEN: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    if let Some(listed) = crate::core::mutex_lock(seen).get(binary) {
        return Some(*listed);
    }
    let mut command = tokio::process::Command::new(binary);
    crate::host::use_desktop_env(command.as_std_mut());
    super::process::use_app_runtime(command.as_std_mut());
    if let Some(home) = binary.parent() {
        command.current_dir(home);
    }
    command
        .arg("--list-devices")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(20), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let listed = lists_backend(&String::from_utf8_lossy(&output.stdout), backend)?;
    crate::core::mutex_lock(seen).insert(binary.to_path_buf(), listed);
    Some(listed)
}

/// Ignore host accelerators such as BLAS when looking for an actual GPU.
fn lists_backend(stdout: &str, backend: &str) -> Option<bool> {
    let mut lines = stdout.lines();
    lines.find(|line| line.trim_start().starts_with("Available devices"))?;
    Some(
        lines
            .take_while(|line| line.starts_with(char::is_whitespace))
            .any(|line| {
                let Some((name, _description)) = line.trim().split_once(": ") else {
                    return false;
                };
                name.starts_with(backend) || (backend == "OpenCL" && name == "GPUOpenCL")
            }),
    )
}

/// The x64 Windows build running under emulation on an ARM PC.
pub(crate) fn runs_x64_on_arm() -> bool {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        native_machine_is_arm64()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        false
    }
}

/// True on Windows ARM, including an x64 process running under emulation.
pub(crate) fn windows_host_is_arm64() -> bool {
    #[cfg(not(windows))]
    {
        false
    }
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        true
    }
    #[cfg(all(windows, not(target_arch = "aarch64")))]
    {
        native_machine_is_arm64() || cpu_looks_like_snapdragon()
    }
}

#[cfg(windows)]
fn system32() -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    std::path::PathBuf::from(root).join("System32")
}

#[cfg(all(windows, any(target_arch = "x86_64", target_arch = "aarch64")))]
fn nvidia_driver_present() -> bool {
    let dir = system32();
    dir.join("nvcuda.dll").is_file() || dir.join("nvml.dll").is_file()
}

#[cfg(all(windows, target_arch = "aarch64"))]
fn adreno_present() -> bool {
    if cpu_looks_like_snapdragon() {
        return true;
    }
    let dir = system32();
    ["qcdx11umd64.dll", "qcdx12umd64.dll", "qcdx12.dll"]
        .iter()
        .any(|name| dir.join(name).is_file())
}

#[cfg(windows)]
fn cpu_looks_like_snapdragon() -> bool {
    use sysinfo::CpuRefreshKind;

    let Ok(mut sys) = sysinfo::System::new() else {
        return false;
    };
    sys.refresh_cpu_list(CpuRefreshKind::nothing());
    let brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_ascii_lowercase())
        .unwrap_or_default();
    brand.contains("qualcomm") || brand.contains("snapdragon") || brand.contains("x elite")
}

#[cfg(all(windows, not(target_arch = "aarch64")))]
fn native_machine_is_arm64() -> bool {
    const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;
    let mut process_machine = 0u16;
    let mut native_machine = 0u16;
    // SAFETY: kernel32, this process, and two stack u16 out-params.
    let ok = unsafe {
        IsWow64Process2(
            GetCurrentProcess(),
            &mut process_machine,
            &mut native_machine,
        )
    };
    ok != 0 && native_machine == IMAGE_FILE_MACHINE_ARM64
}

#[cfg(all(windows, not(target_arch = "aarch64")))]
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn IsWow64Process2(
        hProcess: *mut std::ffi::c_void,
        pProcessMachine: *mut u16,
        pNativeMachine: *mut u16,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_pin_is_absent_on_this_host_or_well_formed() {
        if let Some(pin) = preferred_optional_pin() {
            assert_eq!(pin.os, std::env::consts::OS);
            if pin.arch == std::env::consts::ARCH {
                assert!(pin.accelerator == "CUDA" || pin.accelerator == "OpenCL");
            } else {
                // x64 Windows copy on an ARM PC.
                assert_eq!((pin.arch, pin.accelerator), ("aarch64", "CPU"));
            }
        }
    }

    #[test]
    fn device_list_matches_the_requested_backend() {
        let gpu = "Available devices:\n  Vulkan0: AMD Radeon 780M (RADV PHOENIX) (16384 MiB, 15000 MiB free)\n";
        assert_eq!(lists_backend(gpu, "Vulkan"), Some(true));
        assert_eq!(
            lists_backend(
                "Available devices:\n  GPUOpenCL: Qualcomm Adreno X1-45",
                "OpenCL"
            ),
            Some(true)
        );
        assert_eq!(
            lists_backend("Available devices:\n  CUDA0: NVIDIA RTX Spark", "CUDA"),
            Some(true)
        );
        assert_eq!(
            lists_backend("Available devices:\n  BLAS: CPU", "Vulkan"),
            Some(false)
        );
        assert_eq!(lists_backend("Available devices:\n", "CUDA"), Some(false));
        assert_eq!(
            lists_backend("Available devices:\n  BLAS: CPU acceleration\n", "CUDA"),
            Some(false)
        );
        let trailing = "load_backend: loaded CPU backend\nAvailable devices:\n  Vulkan0: Intel(R) Graphics (ADL GT2) (8000 MiB, 7000 MiB free)\n  Vulkan1: NVIDIA GeForce RTX 4060 (8188 MiB, 8000 MiB free)\nmain: exiting\n";
        assert_eq!(lists_backend(trailing, "Vulkan"), Some(true));
        assert_eq!(lists_backend("error: unknown argument", "Vulkan"), None);
    }

    #[test]
    fn macos_is_not_windows_arm() {
        #[cfg(not(windows))]
        assert!(!windows_host_is_arm64());
        #[cfg(all(windows, target_arch = "aarch64"))]
        assert!(windows_host_is_arm64());
    }
}
