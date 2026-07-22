#!/usr/bin/env pwsh
<#
.SYNOPSIS
    grove installer for Windows — downloads the right prebuilt binary from the
    latest GitHub Release, verifies its sha256, and installs it.

.DESCRIPTION
    One-liner (PowerShell 5.1+ / pwsh):

        irm https://raw.githubusercontent.com/Entelligentsia/grove/main/install.ps1 | iex

    Honors HTTPS_PROXY / HTTP_PROXY / ALL_PROXY (+ NO_PROXY) environment
    variables the same way install.sh (curl/wget) does — set them before
    running the one-liner. Download the script first if you need the -Proxy
    parameter (piping into `iex` doesn't accept script parameters):

        iwr https://raw.githubusercontent.com/Entelligentsia/grove/main/install.ps1 -OutFile install.ps1
        ./install.ps1 -Proxy "http://proxy.corp:8080"

.PARAMETER InstallDir
    Where to put grove.exe. Default: $env:GROVE_INSTALL_DIR, or
    "$env:LOCALAPPDATA\grove\bin".

.PARAMETER Version
    A tag like v0.1.0. Default: $env:GROVE_VERSION, or "latest".

.PARAMETER Proxy
    Explicit proxy URL, e.g. "http://user:pass@proxy.corp:8080". Overrides
    HTTPS_PROXY/HTTP_PROXY/ALL_PROXY. NO_PROXY still applies.
#>
[CmdletBinding()]
param(
    [string]$InstallDir = $(if ($env:GROVE_INSTALL_DIR) { $env:GROVE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "grove\bin" }),
    [string]$Version = $(if ($env:GROVE_VERSION) { $env:GROVE_VERSION } else { "latest" }),
    [string]$Proxy = ""
)

$ErrorActionPreference = "Stop"
$Repo = "Entelligentsia/grove"

function Fail([string]$Message) {
    Write-Error "grove-install: $Message"
    exit 1
}

# --- proxy resolution ---------------------------------------------------
# Mirrors install.sh/curl: an explicit -Proxy always wins; otherwise fall
# back to HTTPS_PROXY, ALL_PROXY, HTTP_PROXY (upper- or lower-case), in that
# order, since every URL fetched here is https://. NO_PROXY (comma or
# semicolon separated hostnames/suffixes, or "*" for "never proxy") suppresses
# the proxy for matching hosts — checked first so it always wins.
function Resolve-ProxyUrl([string]$Uri) {
    $noProxy = if ($env:NO_PROXY) { $env:NO_PROXY } else { $env:no_proxy }
    if ($noProxy) {
        if ($noProxy.Trim() -eq '*') { return $null }
        $targetHost = ([Uri]$Uri).Host
        $entries = $noProxy -split '[;,]' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
        foreach ($entry in $entries) {
            $suffix = $entry.TrimStart('.')
            if ($targetHost -ieq $suffix -or $targetHost.ToLowerInvariant().EndsWith(".$($suffix.ToLowerInvariant())")) {
                return $null
            }
        }
    }

    if ($Proxy) { return $Proxy }

    foreach ($candidate in @($env:HTTPS_PROXY, $env:https_proxy, $env:ALL_PROXY, $env:all_proxy, $env:HTTP_PROXY, $env:http_proxy)) {
        if ($candidate) { return $candidate }
    }
    return $null
}

function Invoke-Download([string]$Uri, [string]$OutFile) {
    $params = @{
        Uri             = $Uri
        OutFile         = $OutFile
        UseBasicParsing = $true
        Headers         = @{ "User-Agent" = "grove-install.ps1" }
    }
    $proxyUrl = Resolve-ProxyUrl -Uri $Uri
    if ($proxyUrl) { $params["Proxy"] = $proxyUrl }
    Invoke-WebRequest @params
}

# --- arch detection -------------------------------------------------------
# Only x86_64-pc-windows-msvc prebuilts are published today (see
# .github/workflows/release.yml); other archs fall back to `cargo install`.
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($arch -ne [System.Runtime.InteropServices.Architecture]::X64) {
    Fail "unsupported arch: $arch (prebuilts cover x86_64; on others use: cargo install --git https://github.com/$Repo grove-cst-cli)"
}
$target = "x86_64-pc-windows-msvc"
$asset = "grove-$target.zip"

if ($Version -eq "latest") {
    $base = "https://github.com/$Repo/releases/latest/download"
} else {
    $base = "https://github.com/$Repo/releases/download/$Version"
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $archivePath = Join-Path $tmp $asset
    Write-Host "grove-install: fetching $asset ($Version)" -ForegroundColor DarkGray
    try {
        Invoke-Download -Uri "$base/$asset" -OutFile $archivePath
    } catch {
        Fail "download failed: $base/$asset ($($_.Exception.Message))"
    }

    # Verify checksum when the sidecar is available.
    $sha256Path = "$archivePath.sha256"
    try {
        Invoke-Download -Uri "$base/$asset.sha256" -OutFile $sha256Path
        $expected = ((Get-Content $sha256Path -Raw) -split '\s+')[0]
        $actual = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash
        if ($expected -and ($expected -ine $actual)) {
            Fail "checksum mismatch: expected $expected, got $actual"
        }
    } catch {
        Write-Host "grove-install: skipping checksum verification ($($_.Exception.Message))" -ForegroundColor DarkYellow
    }

    Expand-Archive -Path $archivePath -DestinationPath $tmp -Force

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path (Join-Path $tmp "grove.exe") -Destination (Join-Path $InstallDir "grove.exe") -Force
} finally {
    Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "grove-install: installed to $InstallDir\grove.exe" -ForegroundColor DarkGray

$pathEntries = $env:Path -split ';'
if ($pathEntries -notcontains $InstallDir) {
    Write-Host "grove-install: add it to PATH:  `$env:Path = `"$InstallDir;`$env:Path`"" -ForegroundColor DarkGray
    Write-Host "grove-install: to persist across sessions:  [Environment]::SetEnvironmentVariable('Path', `"$InstallDir;`$env:Path`", 'User')" -ForegroundColor DarkGray
}

& (Join-Path $InstallDir "grove.exe") --version
