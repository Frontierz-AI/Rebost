; Tauri NSIS hooks (bundle.windows.nsis.installerHooks in tauri.windows.conf.json).

; Visual C++ runtime for this installer's architecture, staged by
; scripts/release-windows.ps1 (Copy-VcRuntime). rebost.exe (Tesseract) and
; llama.cpp's llama-server import MSVCP140.dll and VCRUNTIME140.dll, which a
; fresh Windows may not have. Microsoft allows this app-local copy.
; Defined here, outside the macros, so it names this file's folder.
!define REBOST_VCRT_DIR "${__FILEDIR__}\vcrt"

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

!macro NSIS_HOOK_POSTINSTALL
  SetOutPath "$INSTDIR"
  File /nonfatal "${REBOST_VCRT_DIR}\*.dll"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Before Tauri removes $INSTDIR, which only works once it is empty.
  Delete "$INSTDIR\msvcp140*.dll"
  Delete "$INSTDIR\vcruntime140*.dll"
  Delete "$INSTDIR\concrt140.dll"
!macroend
