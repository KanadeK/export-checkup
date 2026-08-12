[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cargoPrefix = @()
$originalPath = $env:Path
$originalTarget = $env:CARGO_TARGET_DIR
$runningOnWindows = [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT

function Invoke-CargoStep {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )
    Write-Host "[$Name] cargo $($script:cargoPrefix -join ' ') $($Arguments -join ' ')"
    & cargo @script:cargoPrefix @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

Push-Location $projectRoot
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'cargo was not found'
    }

    if ($runningOnWindows -and -not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
        $gnuToolchain = 'stable-x86_64-pc-windows-gnu'
        $toolchainRoot = (& rustc "+$gnuToolchain" --print sysroot).Trim()
        if ($LASTEXITCODE -ne 0 -or -not $toolchainRoot) {
            throw "MSVC link.exe is unavailable and Rust toolchain $gnuToolchain is not installed"
        }
        $gnuBin = Join-Path $toolchainRoot 'lib\rustlib\x86_64-pc-windows-gnu\bin'
        $selfContainedBin = Join-Path $gnuBin 'self-contained'
        $env:Path = "$selfContainedBin;$gnuBin;$env:Path"
        $env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'export-checkup-cargo-target'
        $cargoPrefix = @("+$gnuToolchain")
        Write-Host "[toolchain] using Rust GNU self-contained linker"
    }

    Invoke-CargoStep 'format' @('fmt', '--all', '--', '--check')
    Invoke-CargoStep 'clippy' @('clippy', '--locked', '--all-targets', '--', '-D', 'warnings')
    Invoke-CargoStep 'tests' @('test', '--locked', '--all-targets')
    Invoke-CargoStep 'package' @('package', '--locked', '--allow-dirty')
    Invoke-CargoStep 'release-build' @('build', '--locked', '--release')

    $targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $projectRoot 'target' }
    $binaryName = if ($runningOnWindows) { 'export-checkup.exe' } else { 'export-checkup' }
    $binary = Join-Path (Join-Path $targetRoot 'release') $binaryName
    if (-not (Test-Path -LiteralPath $binary)) {
        throw "release binary was not found at $binary"
    }

    $artifacts = Join-Path $projectRoot '.artifacts\verify'
    New-Item -ItemType Directory -Force -Path $artifacts | Out-Null
    $healthyJsonA = Join-Path $artifacts 'healthy-a.json'
    $healthyJsonB = Join-Path $artifacts 'healthy-b.json'
    $healthyHtml = Join-Path $artifacts 'healthy.html'

    & $binary scan (Join-Path $projectRoot 'examples\healthy') --strict --json $healthyJsonA --html $healthyHtml --force
    if ($LASTEXITCODE -ne 0) { throw "healthy acceptance failed with $LASTEXITCODE" }
    & $binary scan (Join-Path $projectRoot 'examples\healthy') --strict --json $healthyJsonB --force
    if ($LASTEXITCODE -ne 0) { throw "second deterministic scan failed with $LASTEXITCODE" }
    if ((Get-FileHash -Algorithm SHA256 $healthyJsonA).Hash -ne (Get-FileHash -Algorithm SHA256 $healthyJsonB).Hash) {
        throw 'JSON reports are not deterministic'
    }

    & $binary scan (Join-Path $projectRoot 'examples\broken')
    if ($LASTEXITCODE -ne 3) { throw "broken fixture must exit 3, got $LASTEXITCODE" }
    & $binary compare --before (Join-Path $projectRoot 'examples\compare-before') --after (Join-Path $projectRoot 'examples\compare-after')
    if ($LASTEXITCODE -ne 3) { throw "comparison fixture must exit 3, got $LASTEXITCODE" }
    & $binary explain google.sidecar_missing_media
    if ($LASTEXITCODE -ne 0) { throw 'explain command failed' }

    python scripts/secret_scan.py
    if ($LASTEXITCODE -ne 0) { throw 'secret scan failed' }
    git diff --check
    if ($LASTEXITCODE -ne 0) { throw 'git diff whitespace check failed' }

    Write-Host '[verify] all release checks passed'
}
finally {
    Pop-Location
    $env:Path = $originalPath
    $env:CARGO_TARGET_DIR = $originalTarget
}
