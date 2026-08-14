# reset.ps1 — wipe Rebost app data back to first-run state.
# Shelf folders outside app data (e.g. Documents\Rebost) stay on disk.

$ErrorActionPreference = "SilentlyContinue"
$AppId = "io.rebost.app"

Write-Host "Stopping Rebost…"
Get-Process -Name "rebost", "llama-server" | Stop-Process -Force

Start-Sleep -Seconds 1

Write-Host "Removing application state…"
Remove-Item -Recurse -Force "$env:APPDATA\$AppId"
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\$AppId"

Write-Host ""
Write-Host "Done. Rebost is back to first-run state —"
Write-Host "next launch shows onboarding and asks to install the AI model."
Write-Host "Kept on disk: any Shelf folders with your files (e.g. Documents\Rebost)."
