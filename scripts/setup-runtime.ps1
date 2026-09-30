# Downloads a portable Ollama Windows binary into portable/runtime.
# Requires network. Does not modify PATH or install system-wide.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$runtime = Join-Path $root "portable\runtime"
New-Item -ItemType Directory -Force -Path $runtime | Out-Null

Write-Host "This script helps place ollama.exe under portable\runtime\"
Write-Host "Official releases: https://github.com/ollama/ollama/releases"
Write-Host ""
Write-Host "Manual steps (recommended):"
Write-Host "1. Download the Windows zip/exe from Ollama releases"
Write-Host "2. Extract so portable\runtime\ollama.exe exists"
Write-Host "3. Copy your models directory contents into portable\model\"
Write-Host "   (blobs\ and manifests\ for the model you want, e.g. qwen3:4b)"
Write-Host ""
Write-Host "Target runtime folder: $runtime"

if (Test-Path (Join-Path $runtime "ollama.exe")) {
  Write-Host "Found existing ollama.exe"
} elseif (Test-Path (Join-Path $runtime "bin\ollama.exe")) {
  Write-Host "Found existing bin\ollama.exe"
} else {
  Write-Host "ollama.exe not found yet. Place it in the runtime folder above."
  exit 1
}
