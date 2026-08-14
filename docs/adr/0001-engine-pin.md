# ADR 0001: Pin llama.cpp instead of compiling it

- Status: accepted
- Date: 2026-08

Contributors should not need a llama.cpp toolchain. Rebost pins a GitHub release (`ENGINE_BUILD` + per-OS URL + SHA-256 in `engine/pin.rs`) and runs `llama-server` on loopback.

Release installers **bundle that archive** (one OS/arch per artifact) so first chat does not hit GitHub. The binary is still not compiled from source: official builds include shared libraries, so the pin is shipped as a resource and unpacked into app data, not as a single `externalBin` sidecar.

macOS uses Metal builds (arm64 and Intel, separate `.app`s). Windows x64 uses Vulkan; Windows ARM64 uses CPU. Linux pins are in the matrix so the crate compiles. Linux is not a supported platform.
