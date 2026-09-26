# Authenticode-sign llama-server.exe and every DLL inside a staged llama.cpp
# zip, then repack it in place. Upstream ships these unsigned, and antivirus
# heuristics flag them (IDP.Generic). Must run on x64 Windows (Azure SignTool
# is x64); it signs ARM64 binaries too.
#
#   pwsh scripts/sign-engine-windows.ps1
#   pwsh scripts/sign-engine-windows.ps1 -Archive src-tauri/resources/engine/llama-b10964-bin-win-vulkan-x64.zip

param(
    # Defaults to the one zip that scripts/fetch-engine.mjs staged.
    [string]$Archive = ""
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [System.Runtime.InteropServices.OSPlatform]::Windows)) {
    Write-Error "Authenticode signing needs Windows."
}
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($arch -eq [System.Runtime.InteropServices.Architecture]::Arm64) {
    Write-Error "Azure SignTool does not run on Windows ARM. Sign on x64."
}

foreach ($name in @(
        "AZURE_CLIENT_ID",
        "AZURE_CLIENT_SECRET",
        "AZURE_TENANT_ID",
        "AZURE_ARTIFACT_SIGNING_ENDPOINT",
        "AZURE_ARTIFACT_SIGNING_ACCOUNT",
        "AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE"
    )) {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) {
        Write-Error "Missing $name"
    }
}
if (-not (Get-Command artifact-signing-cli -ErrorAction SilentlyContinue)) {
    Write-Error "artifact-signing-cli is not installed."
}

if (-not $Archive) {
    $staged = @(Get-ChildItem -LiteralPath (Join-Path $Root "src-tauri/resources/engine") -Filter *.zip -File)
    if ($staged.Count -ne 1) {
        Write-Error "Expected one staged engine zip, found $($staged.Count). Run node scripts/fetch-engine.mjs first."
    }
    $Archive = $staged[0].FullName
}
$archivePath = (Resolve-Path -LiteralPath $Archive).Path

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("rebost-engine-sign-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Expand-Archive -LiteralPath $archivePath -DestinationPath $work
    $binaries = @(Get-ChildItem -LiteralPath $work -Recurse -File |
        Where-Object { $_.Extension -in @(".exe", ".dll") })
    if (-not ($binaries | Where-Object { $_.Name -eq "llama-server.exe" })) {
        Write-Error "No llama-server.exe in $archivePath"
    }

    foreach ($file in $binaries) {
        & artifact-signing-cli `
            -e $env:AZURE_ARTIFACT_SIGNING_ENDPOINT `
            -a $env:AZURE_ARTIFACT_SIGNING_ACCOUNT `
            -c $env:AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE `
            -d "Rebost" `
            $file.FullName
        if ($LASTEXITCODE -ne 0) {
            Write-Error "artifact-signing-cli exited $LASTEXITCODE on $($file.Name)"
        }
        $sig = Get-AuthenticodeSignature -LiteralPath $file.FullName
        if ($sig.Status -ne "Valid") {
            Write-Error "$($file.Name): Authenticode status is $($sig.Status), not Valid"
        }
    }

    $repacked = "$archivePath.signed.zip"
    Remove-Item -LiteralPath $repacked -ErrorAction SilentlyContinue
    Compress-Archive -Path (Join-Path $work "*") -DestinationPath $repacked -CompressionLevel Optimal
    Move-Item -Force -LiteralPath $repacked -Destination $archivePath
    Write-Host "Signed $($binaries.Count) engine files in $(Split-Path -Leaf $archivePath)"
} finally {
    Remove-Item -Recurse -Force -LiteralPath $work -ErrorAction SilentlyContinue
}
