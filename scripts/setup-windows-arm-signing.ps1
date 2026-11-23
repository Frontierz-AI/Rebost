# Prepare Azure Artifact Signing on Windows on Arm.
#
# The signing client is an x64 signtool dlib (C++/CLI on .NET 8), so Windows on
# Arm runs it under x64 emulation: x64 signtool.exe and the x64 .NET 8 runtime.
# In GitHub Actions this writes SIGNTOOL_PATH, AZURE_CLI_PATH and DOTNET_ROOT_X64
# to GITHUB_ENV for artifact-signing-cli. Elsewhere it prints them.
$ErrorActionPreference = "Stop"

$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($arch -ne [System.Runtime.InteropServices.Architecture]::Arm64) {
    Write-Host "Not Windows on Arm; nothing to set up."
    return
}

$exports = [ordered]@{}

$kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
$signtool = Get-ChildItem -Path $kits -Filter signtool.exe -Recurse -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -eq "x64" } |
    Sort-Object { $_.Directory.Parent.Name } -Descending |
    Select-Object -First 1
if (-not $signtool) { throw "x64 signtool.exe is missing under $kits" }
$exports.SIGNTOOL_PATH = $signtool.FullName

$dotnetX64 = Join-Path $env:ProgramFiles "dotnet\x64"
$hasNet8 = (Test-Path (Join-Path $dotnetX64 "dotnet.exe")) -and
    (& (Join-Path $dotnetX64 "dotnet.exe") --list-runtimes |
        Where-Object { $_ -match '^Microsoft\.NETCore\.App ([89]|\d{2,})\.' })
if (-not $hasNet8) {
    $installer = Join-Path ([System.IO.Path]::GetTempPath()) "dotnet-install.ps1"
    Invoke-WebRequest -Uri "https://dot.net/v1/dotnet-install.ps1" -OutFile $installer
    & $installer -Runtime dotnet -Channel 8.0 -Architecture x64 -InstallDir $dotnetX64 -NoPath
}
$exports.DOTNET_ROOT_X64 = $dotnetX64

# artifact-signing-cli runs `az login` before signtool. The dlib signs with the
# AZURE_* service principal from the environment, so az is optional.
$az = Get-Command az.cmd -ErrorAction SilentlyContinue
if ($az) {
    $exports.AZURE_CLI_PATH = $az.Source
} else {
    $stub = Join-Path ([System.IO.Path]::GetTempPath()) "rebost-az-login.cmd"
    Set-Content -Path $stub -Value "@exit /b 0" -Encoding ascii
    $exports.AZURE_CLI_PATH = $stub
    Write-Host "Azure CLI not found; the dlib uses the AZURE_* environment credential."
}

foreach ($name in $exports.Keys) {
    Write-Host "$name=$($exports[$name])"
    Set-Item -Path "Env:$name" -Value $exports[$name]
    if ($env:GITHUB_ENV) {
        Add-Content -Path $env:GITHUB_ENV -Value "$name=$($exports[$name])"
    }
}
