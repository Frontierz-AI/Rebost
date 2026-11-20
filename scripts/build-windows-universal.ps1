param(
    [Parameter(Mandatory)] [string]$X64,
    [Parameter(Mandatory)] [string]$Arm64,
    [Parameter(Mandatory)] [string]$Output
)
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
foreach ($path in @($X64, $Arm64)) {
    if ((Get-AuthenticodeSignature -LiteralPath $path).Status -ne "Valid") {
        throw "Missing valid Authenticode signature on $path"
    }
}
$makensis = Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'
& $makensis "/DX64=$X64" "/DARM64=$Arm64" "/DOUTPUT=$Output" "$PSScriptRoot\..\src-tauri\windows\universal.nsi"
& artifact-signing-cli -e $env:AZURE_ARTIFACT_SIGNING_ENDPOINT -a $env:AZURE_ARTIFACT_SIGNING_ACCOUNT -c $env:AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE -d Rebost $Output
if ((Get-AuthenticodeSignature -LiteralPath $Output).Status -ne "Valid") {
    throw "Universal installer signing failed"
}
