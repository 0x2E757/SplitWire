param([string]$SdkVersion)
$ErrorActionPreference = 'Stop'
# Compiles tools\abi-probe.c with cl.exe against the Windows SDK and the vendored wireguard.h, runs
# it, and compares every size and offset it prints with the Rust layout tests: an assertion such as
# `assert_eq!(size_of::<Guid>(), 16); // GUID` declares the number Rust relies on for that C name.
# -SdkVersion (such as 10.0.26100.0) picks the SDK; by default vcvars64 takes the newest.
$root = Split-Path -Parent $PSScriptRoot
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'Visual Studio with the x64 C++ tools was not found' }
$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'
$out = Join-Path $root 'target\abi-probe'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$probe = Join-Path $out 'abi-probe.exe'
$object = Join-Path $out 'abi-probe.obj'
$log = Join-Path $out 'cl.txt'
$source = Join-Path $PSScriptRoot 'abi-probe.c'
$include = Join-Path $root 'vendor\wireguard-nt'
# A command file keeps the quoting away from PowerShell's native argument passing.
$script = Join-Path $out 'compile.cmd'
$lines = @(
    '@echo off',
    "set PATH=$(Split-Path -Parent $vswhere);%PATH%",
    "call `"$vcvars`" $SdkVersion >nul || exit /b 1",
    "cl /nologo /W4 /WX /I `"$include`" /Fo`"$object`" /Fe`"$probe`" `"$source`" >`"$log`""
)
Set-Content -LiteralPath $script -Value $lines -Encoding ASCII
& cmd.exe /d /c $script
if ($LASTEXITCODE -ne 0) {
    Get-Content -LiteralPath $log | Write-Output
    throw 'The ABI probe did not compile'
}
$actual = [ordered]@{}
foreach ($line in & $probe) {
    $name, $value = $line -split ' '
    $actual[$name] = [int]$value
}
$declared = [ordered]@{}
$pattern = 'assert_eq!\(\s*(?:size_of::<[^>]+>\(\)|offset_of!\([^)]*\)),\s*(\d+)\s*\);\s*//\s*([A-Za-z_][\w.]*)'
foreach ($file in Get-ChildItem -LiteralPath (Join-Path $root 'src') -Recurse -Filter '*.rs') {
    $text = [IO.File]::ReadAllText($file.FullName)
    foreach ($match in [regex]::Matches($text, $pattern)) {
        $declared[$match.Groups[2].Value] = @([int]$match.Groups[1].Value, $file.Name)
    }
}
$problems = @()
foreach ($name in $declared.Keys) {
    $value, $file = $declared[$name]
    if (-not $actual.Contains($name)) {
        $problems += "${name}: declared in $file, but tools\abi-probe.c does not print it"
    } elseif ($actual[$name] -ne $value) {
        $problems += "${name}: $value in $file, $($actual[$name]) in the SDK"
    }
}
foreach ($name in $actual.Keys) {
    if (-not $declared.Contains($name)) { $problems += "${name}: printed by the probe, but no Rust layout test declares it" }
}
# Interfaces: every GUID and vtable slot the window calls, declared in Rust as
# `Guid::from_u128(0x…); // <header> [class] <Name>` and `: usize = N; // <Interface>.<Method>`,
# compared with the declarations of the installed SDK's headers.
$includeRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Include'
$sdk = if ($SdkVersion) { $SdkVersion } else { (Get-ChildItem -LiteralPath $includeRoot -Directory | Sort-Object Name | Select-Object -Last 1).Name }
$headers = @{}
function Read-Header([string]$name) {
    if (-not $headers.ContainsKey($name)) {
        $text = ''
        foreach ($folder in 'winrt', 'um', 'shared') {
            $path = Join-Path $includeRoot "$sdk\$folder\$name"
            if (Test-Path -LiteralPath $path) {
                $text = [regex]::Replace([IO.File]::ReadAllText($path), '/\*.*?\*/', ' ', 'Singleline')
                break
            }
        }
        $headers[$name] = $text
    }
    $headers[$name]
}
$slotCache = @{}
# The methods of an interface by slot: its bases' first (IUnknown 3, IInspectable 6).
function Get-Methods([string]$name) {
    if ($name -eq 'IUnknown') { return @('QueryInterface', 'AddRef', 'Release') }
    if ($name -eq 'IInspectable') { return @('QueryInterface', 'AddRef', 'Release', 'GetIids', 'GetRuntimeClassName', 'GetTrustLevel') }
    if ($slotCache.ContainsKey($name)) { return $slotCache[$name] }
    foreach ($text in @($headers.Values)) {
        $match = [regex]::Match($text, 'MIDL_INTERFACE\("[^"]+"\)\s*' + $name + '\s*:\s*public\s+(\w+)\s*\{(.*?)\n\s*\};', 'Singleline')
        if ($match.Success) {
            $methods = @(Get-Methods $match.Groups[1].Value)
            foreach ($method in [regex]::Matches($match.Groups[2].Value, 'virtual\s+\w+\s+(?:STDMETHODCALLTYPE\s+)?(\w+)\s*\(')) {
                $methods += $method.Groups[1].Value
            }
            $slotCache[$name] = $methods
            return $methods
        }
        $vtbl = [regex]::Match($text, 'typedef struct ' + $name + 'Vtbl\s*\{(.*?)END_INTERFACE', 'Singleline')
        if ($vtbl.Success) {
            $methods = @([regex]::Matches($vtbl.Groups[1].Value, 'STDMETHODCALLTYPE\s*\*\s*(\w+)') | ForEach-Object { $_.Groups[1].Value })
            $slotCache[$name] = $methods
            return $methods
        }
    }
    $null
}
$guids = 0
$slots = 0
$sources = Get-ChildItem -LiteralPath (Join-Path $root 'src') -Recurse -Filter '*.rs' | ForEach-Object { [IO.File]::ReadAllText($_.FullName) }
foreach ($text in $sources) {
    foreach ($match in [regex]::Matches($text, 'Guid::from_u128\(0x([0-9a-fA-F_]+)\);\s*//\s*(\S+\.h)\s+(class\s+)?(\w+)')) {
        $hex = $match.Groups[1].Value.Replace('_', '').ToLower()
        $guid = '{0}-{1}-{2}-{3}-{4}' -f $hex.Substring(0, 8), $hex.Substring(8, 4), $hex.Substring(12, 4), $hex.Substring(16, 4), $hex.Substring(20)
        $header = Read-Header $match.Groups[2].Value
        $name = $match.Groups[4].Value
        $pattern = if ($match.Groups[3].Success) { 'class\s+DECLSPEC_UUID\("([^"]+)"\)\s*' + $name + '\s*;' } else { 'MIDL_INTERFACE\("([^"]+)"\)\s*' + $name + '\s*:' }
        $found = [regex]::Match($header, $pattern, 'IgnoreCase')
        if (-not $found.Success) { $problems += "${name}: not declared in $($match.Groups[2].Value)" }
        elseif ($found.Groups[1].Value.ToLower() -ne $guid) { $problems += "${name}: GUID $guid in Rust, $($found.Groups[1].Value) in the SDK" }
        $guids++
    }
}
foreach ($header in 'windows.storage.h', 'windows.foundation.collections.h') { Read-Header $header | Out-Null }
foreach ($text in $sources) {
    foreach ($match in [regex]::Matches($text, ':\s*usize\s*=\s*(\d+);\s*//\s*(\w+)\.(\w+)\s*$', 'Multiline')) {
        $slot = [int]$match.Groups[1].Value
        $interface = $match.Groups[2].Value
        $method = $match.Groups[3].Value
        $methods = Get-Methods $interface
        if ($null -eq $methods) { $problems += "${interface}: not found in the SDK headers" }
        elseif ($methods.Count -le $slot -or $methods[$slot] -ne $method) {
            $problems += "$interface.${method}: slot $slot in Rust, $([array]::IndexOf($methods, $method)) in the SDK"
        }
        $slots++
    }
}
if ($problems.Count -gt 0) {
    $problems | ForEach-Object { Write-Output $_ }
    exit 1
}
Write-Output "ABI verified: $($declared.Count) sizes and offsets match the SDK and wireguard.h; $guids GUIDs and $slots vtable slots match the SDK $sdk."
