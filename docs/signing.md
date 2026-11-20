# Signed installers

Windows releases include one signed offline installer with both native installers. The app updater uses the individually signed native packages. The release workflow verifies Authenticode for every executable and DLL in each Windows inference archive. It preserves valid vendor signatures, signs unsigned binaries, and stops if a signature is invalid. This includes the CUDA runtime and the Snapdragon OpenCL engine. Windows copies from before 1.1.1 use a different extraction directory.

Mac releases ship signed and notarized DMGs. In-app updates on all platforms are verified against the updater key.

```bash
pnpm tauri build
```

That writes an unsigned DMG, NSIS, or Linux AppImage/.deb/.rpm for this machine. Gatekeeper will warn on macOS.

Signed builds use a gitignored `.env.signing` file (variable names are in `.env.example`). Those credentials are not in this repository.
