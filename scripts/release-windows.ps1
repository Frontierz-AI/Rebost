# Build the signed Windows NSIS installer. Tauri signs rebost.exe, the
# uninstaller and the installer. On Windows on Arm, run
# scripts/setup-windows-arm-signing.ps1 first: the signing dlib is x64.
param(
    [string]$Target = "",
    # Unsigned local build. Signing the installer afterwards with
    # scripts/sign-windows-nsis.ps1 leaves rebost.exe unsigned, which
    # Smart App Control blocks.
    [switch]$SkipAuthenticode
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [System.Runtime.InteropServices.OSPlatform]::Windows)) {
    Write-Error "Windows NSIS needs Windows. From macOS: gh workflow run release-windows.yml"
}

$envFile = Join-Path $Root ".env.signing"
if (Test-Path $envFile) {
    Get-Content $envFile | ForEach-Object {
        $line = $_.Trim()
        if (-not $line -or $line.StartsWith("#")) { return }
        if ($line -match '^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$') {
            $name = $Matches[1]
            $value = $Matches[2].Trim()
            if ($value.Length -ge 2) {
                $q = $value[0]
                if (($q -eq '"' -or $q -eq "'") -and $value[-1] -eq $q) {
                    $value = $value.Substring(1, $value.Length - 2)
                }
            }
            Set-Item -Path "Env:$name" -Value $value
        }
    }
}

$azureNames = @(
    "AZURE_CLIENT_ID",
    "AZURE_CLIENT_SECRET",
    "AZURE_TENANT_ID",
    "AZURE_ARTIFACT_SIGNING_ENDPOINT",
    "AZURE_ARTIFACT_SIGNING_ACCOUNT",
    "AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE"
)

function Test-AzureSigningEnv {
    foreach ($name in $script:azureNames) {
        $val = [Environment]::GetEnvironmentVariable($name)
        if ([string]::IsNullOrWhiteSpace($val)) {
            return $false
        }
    }
    return $true
}

if ([string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY) -and
    [string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY_PATH)) {
    Write-Error "Set TAURI_SIGNING_PRIVATE_KEY in .env.signing or the GitHub secret (in-app updater)."
}

# Copy the Visual C++ runtime DLLs for the installer's architecture into
# src-tauri/windows/vcrt/. The NSIS hook (windows/installer-hooks.nsh) puts
# them next to rebost.exe: Tesseract (in rebost.exe) and llama.cpp's
# llama-server import MSVCP140.dll and VCRUNTIME140.dll, and a fresh Windows
# may not have them (issue #130). Microsoft allows this app-local copy of the
# redistributable files.
function Copy-VcRuntime {
    param([string]$Arch)

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        throw "vswhere.exe not found; install Visual Studio Build Tools with the C++ workload."
    }
    $vsPath = & $vswhere -latest -products * -property installationPath
    $redistRoot = Join-Path $vsPath "VC\Redist\MSVC"
    $crt = Get-ChildItem -Path $redistRoot -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+\.\d+' } |
        Sort-Object { [version]($_.Name -replace '[^\d.].*$', '') } -Descending |
        ForEach-Object { Get-ChildItem -Path (Join-Path $_.FullName $Arch) -Directory -Filter "Microsoft.VC*.CRT" -ErrorAction SilentlyContinue } |
        Select-Object -First 1
    if (-not $crt) {
        throw "Visual C++ redistributable ($Arch) not found under $redistRoot."
    }
    $dest = Join-Path $Root "src-tauri\windows\vcrt"
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    New-Item -ItemType Directory -Path $dest | Out-Null
    Get-ChildItem -Path $crt.FullName -File |
        Where-Object { $_.Name -match '^(msvcp140.*|vcruntime140.*|concrt140)\.dll$' } |
        Copy-Item -Destination $dest
    foreach ($required in @("msvcp140.dll", "vcruntime140.dll")) {
        if (-not (Test-Path (Join-Path $dest $required))) {
            throw "$required is missing from $($crt.FullName)."
        }
    }
    $names = (Get-ChildItem $dest -File | ForEach-Object Name) -join ", "
    Write-Host "Visual C++ runtime ($Arch) from $($crt.FullName): $names"
}

$vcArch = switch -Wildcard ($Target) {
    "aarch64-*" { "arm64" }
    "x86_64-*" { "x64" }
    default {
        if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }
    }
}
Copy-VcRuntime -Arch $vcArch

if ($Target) {
    rustup target add $Target
    Write-Host "Staging engine pin for $Target…"
    node scripts/fetch-engine.mjs --triple=$Target
    $prefix = Join-Path $Root "src-tauri/target/$Target/release/bundle/nsis"
} else {
    Write-Host "Staging engine pin for this Windows host…"
    node scripts/fetch-engine.mjs
    $prefix = Join-Path $Root "src-tauri/target/release/bundle/nsis"
}

function Invoke-NsisBuild {
    param([bool]$Sign)

    $config = @{
        bundle = @{
            createUpdaterArtifacts = $true
        }
    }
    if ($Sign) {
        # beforeBuildCommand signs llama-server.exe and its DLLs in the staged zip.
        $env:REBOST_SIGN_ENGINE = "1"
        $signCommand = @(
            "artifact-signing-cli",
            "-e", $env:AZURE_ARTIFACT_SIGNING_ENDPOINT,
            "-a", $env:AZURE_ARTIFACT_SIGNING_ACCOUNT,
            "-c", $env:AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE,
            "-d", "Rebost",
            "%1"
        ) -join " "
        $config.bundle.windows = @{
            signCommand = $signCommand
        }
    } else {
        Remove-Item Env:REBOST_SIGN_ENGINE -ErrorAction SilentlyContinue
    }
    $configPath = Join-Path $env:TEMP "rebost-tauri-windows-sign.json"
    $json = $config | ConvertTo-Json -Depth 6 -Compress
    [System.IO.File]::WriteAllText($configPath, $json)

    if ($Target) {
        pnpm tauri build --target $Target --bundles nsis --config $configPath
    } else {
        pnpm tauri build --bundles nsis --config $configPath
    }
    if ($LASTEXITCODE -ne 0) {
        throw "tauri build exited $LASTEXITCODE"
    }
}

function Test-ExesAuthenticodeValid {
    param([string]$Dir)
    $ok = $true
    Get-ChildItem $Dir -Filter *.exe -ErrorAction Stop | ForEach-Object {
        $sig = Get-AuthenticodeSignature $_.FullName
        Write-Host "$($_.Name): $($sig.Status) $($sig.SignerCertificate.Subject)"
        if ($sig.Status -ne "Valid") {
            $ok = $false
        }
    }
    return $ok
}

if ($SkipAuthenticode) {
    if (-not $env:REBOST_SIGNED_ENGINE_DIR) { throw "Missing signed engines for the ARM64 installer" }
    Invoke-NsisBuild -Sign $false
} else {
    if (-not (Test-AzureSigningEnv)) { throw "Missing Azure signing configuration" }
    Invoke-NsisBuild -Sign $true
    # Tauri signs the patched rebost.exe it bundles, then restores the unsigned
    # one under target/. The workflow's install test checks the installed files.
    if (-not (Test-ExesAuthenticodeValid -Dir $prefix)) { throw "Invalid installer signature" }
}

Write-Host ""
Write-Host "Artifacts in $prefix"
Get-ChildItem $prefix -ErrorAction Stop | Format-Table Name, Length

$hostTriple = ((rustc -vV) | Where-Object { $_ -match '^host: ' }) -replace '^host:\s+', ''
$triple = if ($Target) { $Target } else { $hostTriple.Trim() }
Write-Host ""
node scripts/latest-json.mjs --bundle-dir $prefix --triple $triple
Write-Host "Attach the NSIS exe, .sig, and dist/latest.json to the GitHub Release."
Write-Host "Merge fragments from every platform: node scripts/latest-json.mjs --combine"
