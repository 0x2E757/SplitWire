param([Parameter(Mandatory=$true)][string]$Pixels)
# Encodes the app icon's large sizes that build\main.rs left in -Pixels (<size>.bgra, 0xAARRGGBB
# rows top-down) as PNG files in assets\icon, with the fingerprint of those pixels.
# The build script takes the PNG files only while the fingerprint still matches the app icon.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$projectRoot = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $projectRoot 'assets\icon'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$total = 0
foreach ($file in Get-ChildItem -LiteralPath $Pixels -Filter '*.bgra') {
    $size = [int]$file.BaseName
    $bytes = [IO.File]::ReadAllBytes($file.FullName)
    if ($bytes.Length -ne $size * $size * 4) { throw "$($file.Name) is not $size x $size pixels" }
    $bitmap = New-Object Drawing.Bitmap $size, $size, ([Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
        $rectangle = New-Object Drawing.Rectangle 0, 0, $size, $size
        $data = $bitmap.LockBits($rectangle, [Drawing.Imaging.ImageLockMode]::WriteOnly, $bitmap.PixelFormat)
        try {
            for ($row = 0; $row -lt $size; $row++) {
                [Runtime.InteropServices.Marshal]::Copy($bytes, $row * $size * 4, [IntPtr]($data.Scan0.ToInt64() + $row * $data.Stride), $size * 4)
            }
        } finally { $bitmap.UnlockBits($data) }
        $path = Join-Path $destination "$size.png"
        $bitmap.Save($path, [Drawing.Imaging.ImageFormat]::Png)
    } finally { $bitmap.Dispose() }
    $length = (Get-Item -LiteralPath $path).Length
    $total += $length
    Write-Output "$size.png: $length bytes"
}
Copy-Item -LiteralPath (Join-Path $Pixels 'pixels.fnv') -Destination $destination
Write-Output "Total: $total bytes"
