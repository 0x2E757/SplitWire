param([string]$Root = (Split-Path $PSScriptRoot -Parent))
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
# Every Markdown file of the repository stays within its size, in characters:
# the root AGENTS.md 5000, the root README.md 10000, any other file 3000.
$limits = @{ 'AGENTS.md' = 5000; 'README.md' = 10000 }
$default = 3000
$files = @(git -C $Root -c core.quotepath=off ls-files --cached --others --exclude-standard -- '*.md')
if ($LASTEXITCODE -ne 0) { throw 'git ls-files failed' }
$over = @()
$checked = 0
foreach ($file in $files) {
    $path = Join-Path $Root $file
    if (-not (Test-Path -LiteralPath $path)) { continue }
    $text = [IO.File]::ReadAllText($path, [Text.Encoding]::UTF8).Replace("`r`n", "`n")
    # A character is a code point: a surrogate pair counts once.
    $length = $text.Length - [regex]::Matches($text, '[\uDC00-\uDFFF]').Count
    $limit = if ($limits.ContainsKey($file)) { $limits[$file] } else { $default }
    $checked++
    if ($length -gt $limit) {
        $over += '{0}: {1} characters, limit {2}, {3} over' -f $file, $length, $limit, ($length - $limit)
    }
}
if ($over.Count -gt 0) {
    $over | ForEach-Object { Write-Output $_ }
    Write-Output ''
    Write-Output 'Bring each file within its limit: move a self-contained part into another file and link to it,'
    Write-Output 'or remove what is not essential, that is, only what no reader or agent will miss.'
    exit 1
}
Write-Output "Markdown sizes verified: $checked files within their limits."
