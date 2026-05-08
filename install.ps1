# Install `rag` from the latest GitHub release.
#
# Usage:
#   irm https://github.com/mario-vanhecke/rag/raw/main/install.ps1 | iex
#
# Environment overrides:
#   $env:RAG_VERSION    pin a specific version (default: latest)
#   $env:RAG_PREFIX     install dir (default: %LOCALAPPDATA%\rag\bin)

#Requires -Version 5

$ErrorActionPreference = 'Stop'

$repo    = 'mario-vanhecke/rag'
$version = if ($env:RAG_VERSION) { $env:RAG_VERSION } else { 'latest' }

function Write-Ok    ($msg) { Write-Host "ok    $msg"   -ForegroundColor Green }
function Write-Note  ($msg) { Write-Host "note  $msg"   -ForegroundColor Yellow }
function Write-Err   ($msg) { Write-Host "error $msg"   -ForegroundColor Red; exit 1 }
function Write-Bold  ($msg) { Write-Host $msg -ForegroundColor White }

# ---------- detect arch ----------
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    'AMD64' { $target = 'x86_64-pc-windows-msvc' }
    'ARM64' { Write-Err "Windows on ARM64 is not yet packaged; build from source via 'cargo install --git https://github.com/$repo rag-cli'" }
    default { Write-Err "unsupported architecture: $arch" }
}

Write-Bold "Installing rag for Windows ($arch)"

# ---------- install prefix ----------
$prefix = if ($env:RAG_PREFIX) { $env:RAG_PREFIX } else { Join-Path $env:LOCALAPPDATA 'rag\bin' }
New-Item -ItemType Directory -Force -Path $prefix | Out-Null
Write-Ok "install prefix: $prefix"

# ---------- resolve URL ----------
if ($version -eq 'latest') {
    $assetUrl = "https://github.com/$repo/releases/latest/download/rag-$target.zip"
} else {
    $assetUrl = "https://github.com/$repo/releases/download/$version/rag-$target.zip"
}

# ---------- download + extract ----------
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("rag-install-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    $zip = Join-Path $tmp 'rag.zip'
    Write-Ok "downloading $assetUrl"
    try {
        Invoke-WebRequest -Uri $assetUrl -OutFile $zip -UseBasicParsing
    } catch {
        Write-Err "download failed. check that a release exists at $assetUrl"
    }

    Write-Ok "extracting"
    Expand-Archive -LiteralPath $zip -DestinationPath $tmp -Force

    # Find the binary in the extracted tree.
    $binSrc = $null
    foreach ($candidate in @((Join-Path $tmp 'rag.exe'), (Join-Path $tmp "rag-$target\rag.exe"))) {
        if (Test-Path $candidate) { $binSrc = $candidate; break }
    }
    if (-not $binSrc) { Write-Err "binary 'rag.exe' not found inside the zip" }

    Move-Item -Force -Path $binSrc -Destination (Join-Path $prefix 'rag.exe')
    Write-Ok "installed: $prefix\rag.exe"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

# ---------- PATH hint ----------
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$onPath   = ($userPath -split ';') -contains $prefix
if (-not $onPath) {
    Write-Note "$prefix is not on your PATH. Adding it for the current user."
    $newPath = if ($userPath) { "$userPath;$prefix" } else { $prefix }
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    Write-Note "Open a new terminal for the change to take effect."
}

# ---------- pandoc heads-up ----------
$pandoc = Get-Command pandoc -ErrorAction SilentlyContinue
if (-not $pandoc) {
    Write-Note "pandoc is not installed. DOCX/PDF support requires it. Markdown/text vaults work without it."
    Write-Host  "        winget install pandoc" -ForegroundColor DarkGray
}

Write-Host ''
Write-Bold 'Next:'
Write-Host '  rag --version'
Write-Host '  rag init .'
Write-Host '  rag add <path>'
Write-Host '  rag index'
Write-Host '  rag search "<query>"'
