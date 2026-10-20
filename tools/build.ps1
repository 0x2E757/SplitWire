param([switch]$SkipTests, [string]$Destination)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not $SkipTests) {
        & cargo fmt --check
        if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed' }
        & cargo test --offline
        if ($LASTEXITCODE -ne 0) { throw 'Tests failed' }
        & cargo clippy --offline --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Clippy failed' }
    }
    & cargo build --release --offline
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $destination = if ($Destination) { $Destination } else { Join-Path $projectRoot 'target\package' }
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    . (Join-Path $PSScriptRoot 'package-names.ps1')
    foreach ($name in $PackageNames.Keys) {
        Copy-Item -LiteralPath (Join-Path $projectRoot "target\release\$name") -Destination (Join-Path $destination $PackageNames[$name])
    }
    # The licence allows the DLL only unmodified and under its own name, with its licence beside it.
    $wireguard = Join-Path $projectRoot 'vendor\wireguard-nt'
    Copy-Item -LiteralPath (Join-Path $wireguard 'wireguard.dll') -Destination $destination
    Copy-Item -LiteralPath (Join-Path $wireguard 'LICENSE.txt') -Destination (Join-Path $destination 'wireguard-nt-LICENSE.txt')
    Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $destination
    Write-Output "Package: $destination"
} finally {
    Pop-Location
}
