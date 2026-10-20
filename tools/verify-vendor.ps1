param([string]$Root = (Split-Path $PSScriptRoot -Parent))
$ErrorActionPreference = 'Stop'
# Every vendored file is listed with its SHA-256 in its folder's README.md, and nothing else lies
# there; wireguard.dll carries a valid Authenticode signature by WireGuard LLC.
$folder = Join-Path $Root 'vendor\wireguard-nt'
$readme = [IO.File]::ReadAllText((Join-Path $folder 'README.md'))
$listed = @{}
foreach ($row in [regex]::Matches($readme, '(?m)^\|\s*([\w.-]+)\s*\|.*\|\s*([0-9A-Fa-f]{64})\s*\|\s*$')) {
    $listed[$row.Groups[1].Value] = $row.Groups[2].Value.ToUpperInvariant()
}
$problems = @()
foreach ($name in $listed.Keys) {
    $path = Join-Path $folder $name
    if (-not (Test-Path -LiteralPath $path)) { $problems += "${name}: listed, but missing"; continue }
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    if ($hash -ne $listed[$name]) { $problems += "${name}: SHA-256 $hash, README says $($listed[$name])" }
}
foreach ($file in Get-ChildItem -LiteralPath $folder -File) {
    if ($file.Name -ne 'README.md' -and -not $listed.ContainsKey($file.Name)) { $problems += "$($file.Name): not listed in README.md" }
}
$signature = Get-AuthenticodeSignature -LiteralPath (Join-Path $folder 'wireguard.dll')
$signer = if ($signature.SignerCertificate) { $signature.SignerCertificate.GetNameInfo('SimpleName', $false) } else { '' }
if ($signature.Status -ne 'Valid' -or $signer -ne 'WireGuard LLC') {
    $problems += "wireguard.dll: signature $($signature.Status), signer '$signer', expected a valid one by WireGuard LLC"
}
if ($problems.Count -gt 0) {
    $problems | ForEach-Object { Write-Output $_ }
    exit 1
}
Write-Output "Vendored files verified: $($listed.Count) hashes match, wireguard.dll is signed by $signer."
