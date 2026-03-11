[CmdletBinding()]
param(
    [string]$ProjectRoot = ".",
    [string]$OutputPath = ".ai/index.json"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-IsoUtcNow {
    return [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
}

function Normalize-PathValue {
    param([string]$Value)
    $normalized = $Value.Replace('\', '/')
    $normalized = $normalized -replace '^\./', ''
    $normalized = $normalized -replace '^/', ''
    return $normalized
}

function Get-PlatformName {
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) { return "windows" }
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Linux)) { return "linux" }
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::OSX)) { return "macos" }
    return "unknown"
}

function Detect-LanguageStack {
    param([string]$RootPath)
    $set = New-Object System.Collections.Generic.HashSet[string]
    if (Test-Path -LiteralPath (Join-Path $RootPath "Cargo.toml")) { [void]$set.Add("Rust") }
    if (Test-Path -LiteralPath (Join-Path $RootPath "package.json")) { [void]$set.Add("Node.js") }
    if (Test-Path -LiteralPath (Join-Path $RootPath "tsconfig.json")) { [void]$set.Add("TypeScript") }
    if (Test-Path -LiteralPath (Join-Path $RootPath "pyproject.toml")) { [void]$set.Add("Python") }
    if (Test-Path -LiteralPath (Join-Path $RootPath "go.mod")) { [void]$set.Add("Go") }
    $hasVue = Get-ChildItem -Path $RootPath -Recurse -File -Filter "*.vue" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($hasVue) { [void]$set.Add("Vue") }
    if ($set.Count -eq 0) { [void]$set.Add("Unknown") }
    return @($set | Sort-Object)
}

function Is-ExcludedPath {
    param([string]$RelativePath)
    $rel = Normalize-PathValue $RelativePath
    $patterns = @(
        '^node_modules/', '^\.git/', '^dist/', '^build/', '^target/', '^coverage/',
        '^refer/', '^tests/skill-triggering/output/', '^\.ai/analysis/', '^\.ai/insights/',
        '^\.ai/workflow/audit\.jsonl$', '^\.worktrees/', '^worktrees/'
    )
    foreach ($pattern in $patterns) {
        if ($rel -match $pattern) { return $true }
    }
    return $false
}

function Get-Fingerprint {
    param([System.IO.FileInfo]$File)
    return "stat:{0}-{1}" -f $File.Length, [int64][Math]::Floor($File.LastWriteTimeUtc.Subtract([DateTime]'1970-01-01').TotalSeconds)
}

$root = [System.IO.Path]::GetFullPath($ProjectRoot)
$outputAbs = if ([System.IO.Path]::IsPathRooted($OutputPath)) { $OutputPath } else { Join-Path $root $OutputPath }
$outputDir = Split-Path -Parent $outputAbs
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

$files = [ordered]@{}
$total = 0
$indexed = 0

Get-ChildItem -Path $root -Recurse -File -Force | ForEach-Object {
    $total += 1
    $rel = Normalize-PathValue ([System.IO.Path]::GetRelativePath($root, $_.FullName))
    if ([string]::IsNullOrWhiteSpace($rel)) { return }
    if (Is-ExcludedPath $rel) { return }

    $lineCount = 0
    try { $lineCount = (Get-Content -LiteralPath $_.FullName -Encoding utf8 -ErrorAction Stop | Measure-Object -Line).Lines }
    catch { $lineCount = 0 }

    $files[$rel] = [ordered]@{
        path = $rel
        name = $_.Name
        exists = $true
        size_bytes = [int64]$_.Length
        line_count = [int64]$lineCount
        encoding = "utf-8"
        last_modified = $_.LastWriteTimeUtc.ToString("yyyy-MM-ddTHH:mm:ssZ")
        last_modified_epoch = [int64][Math]::Floor($_.LastWriteTimeUtc.Subtract([DateTime]'1970-01-01').TotalSeconds)
        fingerprint = Get-Fingerprint $_
        previous_fingerprint = $null
        status = "MATCH"
        change_type = "unchanged"
        role = "unknown"
        modify_prob = "Low"
        understanding = [ordered]@{
            confidence = 0.0
            summary = "Index baseline created; detailed understanding pending."
            interfaces = @()
            constraints = @()
            dependencies = @()
            evidence = @()
            blind_spots = @("Not yet analyzed with sy-code-insight.")
        }
    }
    $indexed += 1
}

$index = [ordered]@{
    schema_name = "sy.index"
    schema_version = "1.0.0"
    generated_at = Get-IsoUtcNow
    updated_at = Get-IsoUtcNow
    project = [ordered]@{
        root = $root.Replace('\', '/')
        platform = Get-PlatformName
        language_stack = @(Detect-LanguageStack $root)
    }
    stats = [ordered]@{
        scanned_files = $total
        indexed_files = $indexed
    }
    files = $files
}

$jsonText = ($index | ConvertTo-Json -Depth 8) + [Environment]::NewLine
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($outputAbs, $jsonText, $utf8NoBom)
Write-Host "Index generated: $outputAbs ($indexed files)"
