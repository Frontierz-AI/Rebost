# Signed NSIS installer via Azure Artifact Signing.
# Must run on Windows (SignTool). From macOS use: gh workflow run release-windows.yml
#
#   pwsh scripts/release-windows.ps1
#   pwsh scripts/release-windows.ps1 -Target aarch64-pc-windows-msvc

param(
    [string]$Target = ""
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [System.Runtime.InteropServices.OSPlatform]::Windows)) {
    Write-Error "Authenticode signing needs Windows. From macOS: gh workflow run release-windows.yml"
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

foreach ($name in @(
        "AZURE_CLIENT_ID",
        "AZURE_CLIENT_SECRET",
        "AZURE_TENANT_ID",
        "AZURE_ARTIFACT_SIGNING_ENDPOINT",
        "AZURE_ARTIFACT_SIGNING_ACCOUNT",
        "AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE"
    )) {
    $val = [Environment]::GetEnvironmentVariable($name)
    if ([string]::IsNullOrWhiteSpace($val)) {
        Write-Error "Set $name in .env.signing (see .env.example)."
    }
}

if ([string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY) -and
    [string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY_PATH)) {
    Write-Error "Set TAURI_SIGNING_PRIVATE_KEY in .env.signing or the GitHub secret (in-app updater)."
}

if (-not (Get-Command artifact-signing-cli -ErrorAction SilentlyContinue)) {
    Write-Error "Install with: cargo install artifact-signing-cli"
}

$signCommand = @(
    "artifact-signing-cli",
    "-e", $env:AZURE_ARTIFACT_SIGNING_ENDPOINT,
    "-a", $env:AZURE_ARTIFACT_SIGNING_ACCOUNT,
    "-c", $env:AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE,
    "-d", "Rebost",
    "%1"
) -join " "

$configPath = Join-Path $env:TEMP "rebost-tauri-windows-sign.json"
$config = @{
    bundle = @{
        createUpdaterArtifacts = $true
        windows = @{
            signCommand = $signCommand
        }
    }
}
$json = $config | ConvertTo-Json -Depth 6 -Compress
[System.IO.File]::WriteAllText($configPath, $json)

if ($Target) {
    rustup target add $Target
    Write-Host "Staging engine pin for $Target…"
    node scripts/fetch-engine.mjs --triple=$Target
    Write-Host "Building signed NSIS for $Target…"
    pnpm tauri build --target $Target --bundles nsis --config $configPath
    $prefix = Join-Path $Root "src-tauri/target/$Target/release/bundle/nsis"
} else {
    Write-Host "Staging engine pin for this Windows host…"
    node scripts/fetch-engine.mjs
    Write-Host "Building signed NSIS for this Windows host…"
    pnpm tauri build --bundles nsis --config $configPath
    $prefix = Join-Path $Root "src-tauri/target/release/bundle/nsis"
}

Write-Host ""
Write-Host "Artifacts in $prefix"
Get-ChildItem $prefix -ErrorAction Stop | Format-Table Name, Length

Get-ChildItem $prefix -Filter *.exe | ForEach-Object {
    $sig = Get-AuthenticodeSignature $_.FullName
    Write-Host "$($_.Name): $($sig.Status) $($sig.SignerCertificate.Subject)"
    if ($sig.Status -eq "NotSigned") {
        Write-Error "$($_.Name) is not signed"
    }
}

$hostTriple = ((rustc -vV) | Where-Object { $_ -match '^host: ' }) -replace '^host:\s+', ''
$triple = if ($Target) { $Target } else { $hostTriple.Trim() }
Write-Host ""
node scripts/latest-json.mjs --bundle-dir $prefix --triple $triple
Write-Host "Attach the NSIS exe, .sig, and dist/latest.json to the GitHub Release."
Write-Host "Merge fragments from every platform: node scripts/latest-json.mjs --combine"
