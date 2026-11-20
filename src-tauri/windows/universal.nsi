Unicode true
Name "Rebost"
OutFile "${OUTPUT}"
RequestExecutionLevel user
SilentInstall silent
SetCompress off
Icon "..\icons\icon.ico"
!include "x64.nsh"
!include "FileFunc.nsh"

Section
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  ${If} ${IsNativeARM64}
    File /oname=Rebost-setup.exe "${ARM64}"
  ${ElseIf} ${IsNativeAMD64}
    File /oname=Rebost-setup.exe "${X64}"
  ${Else}
    MessageBox MB_ICONSTOP|MB_OK "Rebost requires 64-bit Windows (Intel, AMD or ARM64)." /SD IDOK
    SetErrorLevel 1633
    Quit
  ${EndIf}
  ${GetParameters} $0
  ClearErrors
  ExecWait '"$PLUGINSDIR\Rebost-setup.exe" $0' $1
  IfErrors 0 +2
    StrCpy $1 1603
  SetErrorLevel $1
SectionEnd
