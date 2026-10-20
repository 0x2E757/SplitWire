param([switch]$SkipTests)
# Builds the release files in target\dist: SplitWire-Setup-<version>.exe (Inno Setup,
# installer\splitwire.iss) and SHA256SUMS.txt. The package is staged in target\dist\package,
# so a copy running from target\package keeps running.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $version = (Select-String -LiteralPath 'Cargo.toml' -Pattern '^version = "(.+)"$' | Select-Object -First 1).Matches[0].Groups[1].Value
    $dist = Join-Path $projectRoot 'target\dist'
    $package = Join-Path $dist 'package'
    if (Test-Path -LiteralPath $dist) { Remove-Item -LiteralPath $dist -Recurse -Force }
    & (Join-Path $PSScriptRoot 'build.ps1') -SkipTests:$SkipTests -Destination $package

    # The Setup icon: PNG images in an .ico, the small sizes scaled from the largest.
    Add-Type -AssemblyName System.Drawing
    $images = @()
    $large = [Drawing.Image]::FromFile((Join-Path $projectRoot 'assets\icon\256.png'))
    try {
        foreach ($size in 16, 24, 32, 48, 64) {
            $bitmap = New-Object Drawing.Bitmap $size, $size
            $graphics = [Drawing.Graphics]::FromImage($bitmap)
            $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.DrawImage($large, 0, 0, $size, $size)
            $graphics.Dispose()
            $stream = New-Object IO.MemoryStream
            $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
            $bitmap.Dispose()
            $images += [pscustomobject]@{ Size = $size; Bytes = $stream.ToArray() }
        }
    } finally { $large.Dispose() }
    $images += [pscustomobject]@{ Size = 256; Bytes = [IO.File]::ReadAllBytes((Join-Path $projectRoot 'assets\icon\256.png')) }
    $icon = Join-Path $dist 'setup.ico'
    $writer = New-Object IO.BinaryWriter ([IO.File]::Create($icon))
    try {
        $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$images.Count)
        $offset = 6 + 16 * $images.Count
        foreach ($image in $images) {
            $side = if ($image.Size -ge 256) { 0 } else { $image.Size }
            $writer.Write([byte]$side); $writer.Write([byte]$side); $writer.Write([byte]0); $writer.Write([byte]0)
            $writer.Write([uint16]1); $writer.Write([uint16]32)
            $writer.Write([uint32]$image.Bytes.Length); $writer.Write([uint32]$offset)
            $offset += $image.Bytes.Length
        }
        foreach ($image in $images) { $writer.Write($image.Bytes) }
    } finally { $writer.Dispose() }

    $compiler = @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
    ) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $compiler) { throw 'Inno Setup 6 is not installed: winget install JRSoftware.InnoSetup --scope user' }
    & $compiler /Q "/DAppVersion=$version" "/DSource=$package" "/DIcon=$icon" "/DOutputDir=$dist" (Join-Path $projectRoot 'installer\splitwire.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Setup build failed' }

    $sums = Get-ChildItem -LiteralPath $dist -Filter 'SplitWire-*.exe' | ForEach-Object {
        "$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
    }
    [IO.File]::WriteAllLines((Join-Path $dist 'SHA256SUMS.txt'), [string[]]$sums)
    Get-ChildItem -LiteralPath $dist -File | Select-Object Name, Length
} finally {
    Pop-Location
}
