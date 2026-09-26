; Tauri NSIS hooks (bundle.windows.nsis.installerHooks in tauri.windows.conf.json).

!macro NSIS_HOOK_PREINSTALL
  !if "${ARCH}" == "x64"
    ; The regular installer on an ARM PC runs Rebost under emulation. Point a
    ; person installing by hand to the ARM download. Updates, passive and
    ; silent installs carry on, so an older x64 copy can still update itself
    ; (the app then offers the ARM build through the updater).
    ${If} ${IsNativeARM64}
    ${AndIf} $UpdateMode <> 1
    ${AndIf} $PassiveMode <> 1
    ${AndIfNot} ${Silent}
      ${If} ${Cmd} `MessageBox MB_YESNO|MB_ICONINFORMATION "This is Rebost for regular Windows PCs, and this PC has an ARM processor.$\r$\n$\r$\nRebost for Windows (ARM) runs faster and more reliably here. Open the download page and stop this installation?" /SD IDNO IDYES`
        ExecShell "open" "https://github.com/Frontierz-AI/Rebost/releases/latest"
        Quit
      ${EndIf}
    ${EndIf}
  !endif
!macroend
