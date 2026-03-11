[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ReadmePath,

    [string]$ProjectRoot,

    [int]$MtimeToleranceSeconds = 2,

    [switch]$AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-NormalizedPath {
    param(
        [string]$BasePath,
        [string]$InputPath
    )

    if ([System.IO.Path]::IsPathRooted($InputPath)) {
        return [System.IO.Path]::GetFullPath($InputPath)
    }

    return [System.IO.Path]::GetFullPath((Join-Path -Path $BasePath -ChildPath $InputPath))
}

function Parse-SourceManifestFromMarkdown {
    param([string]$Markdown)

    $blocks = [System.Text.RegularExpressions.Regex]::Matches(
        $Markdown,
        '(?s)```(?:yaml|yml)?\s*(.*?)```'
    )

    $targetBlock = $null
    foreach ($block in $blocks) {
        $value = $block.Groups[1].Value
        if ($value -match "(?m)^\s*source_manifest\s*:") {
            $targetBlock = $value
            break
        }
    }

    if (-not $targetBlock) {
        return $null
    }

    $manifest = @{
        schema = $null
        generated_at = $null
        base_ref = $null
        files = @()
    }

    $current = $null
    $lines = $targetBlock -split '\r?\n'
    foreach ($rawLine in $lines) {
        $line = $rawLine.Trim()
        if (-not $line) { continue }

        if ($line -match "^source_manifest\s*:") { continue }

        if ($line -match "^schema\s*:\s*(.+)$") {
            $manifest.schema = $matches[1].Trim().Trim("'", '"')
            continue
        }

        if ($line -match "^generated_at\s*:\s*(.+)$") {
            $manifest.generated_at = $matches[1].Trim().Trim("'", '"')
            continue
        }

        if ($line -match "^base_ref\s*:\s*(.+)$") {
            $manifest.base_ref = $matches[1].Trim().Trim("'", '"')
            continue
        }

        if ($line -match "^-?\s*path\s*:\s*(.+)$") {
            if ($current) {
                $manifest.files += $current
            }

            $current = @{
                path = $matches[1].Trim().Trim("'", '"')
                fingerprint = $null
            }
            continue
        }

        if ($line -match "^fingerprint\s*:\s*(.+)$") {
            if (-not $current) {
                $current = @{
                    path = $null
                    fingerprint = $null
                }
            }
            $current.fingerprint = $matches[1].Trim().Trim("'", '"')
            continue
        }
    }

    if ($current) {
        $manifest.files += $current
    }

    return $manifest
}

function Get-RenameMap {
    param([string]$RepoRoot)

    $renameMap = @{}
    $gitDiffOutput = $null

    try {
        $gitDiffOutput = & git -C $RepoRoot diff --name-status -M 2>$null
        if ($LASTEXITCODE -ne 0) {
            return $renameMap
        }
    } catch {
        return $renameMap
    }

    foreach ($line in $gitDiffOutput) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }

        $parts = $line -split "`t"
        if ($parts.Count -lt 3) { continue }
        if (-not $parts[0].StartsWith("R")) { continue }

        $oldPath = $parts[1].Trim()
        $newPath = $parts[2].Trim()
        if ($oldPath -and $newPath) {
            $renameMap[$oldPath] = $newPath
        }
    }

    return $renameMap
}

function Get-FileStateByStat {
    param(
        [hashtable]$Entry,
        [string]$RootPath,
        [int]$ToleranceSeconds,
        [hashtable]$RenameMap
    )

    $path = $Entry.path
    $fingerprint = $Entry.fingerprint
    $absolutePath = Resolve-NormalizedPath -BasePath $RootPath -InputPath $path

    if (-not (Test-Path -LiteralPath $absolutePath)) {
        if ($RenameMap.ContainsKey($path)) {
            $renamedPath = $RenameMap[$path]
            $renamedAbs = Resolve-NormalizedPath -BasePath $RootPath -InputPath $renamedPath
            if (Test-Path -LiteralPath $renamedAbs) {
                return [pscustomobject]@{
                    path = $path
                    current_path = $renamedPath
                    status = "RENAMED"
                    reason = "File path renamed by git diff -M"
                    expected_fingerprint = $fingerprint
                    current_fingerprint = $null
                }
            }
        }

        return [pscustomobject]@{
            path = $path
            current_path = $null
            status = "DELETED"
            reason = "File not found"
            expected_fingerprint = $fingerprint
            current_fingerprint = $null
        }
    }

    $item = Get-Item -LiteralPath $absolutePath
    if ($item.PSIsContainer) {
        return [pscustomobject]@{
            path = $path
            current_path = $path
            status = "UNKNOWN"
            reason = "Manifest target is directory, expected file"
            expected_fingerprint = $fingerprint
            current_fingerprint = $null
        }
    }

    $currentSize = [int64]$item.Length
    $currentMtime = [DateTimeOffset]$item.LastWriteTimeUtc
    $currentMtimeEpoch = [int64]$currentMtime.ToUnixTimeSeconds()
    $currentMarker = "stat:$currentSize-$currentMtimeEpoch"

    if (-not $fingerprint -or -not $fingerprint.StartsWith("stat:")) {
        return [pscustomobject]@{
            path = $path
            current_path = $path
            status = "UNKNOWN"
            reason = "Unsupported fingerprint format, expected stat:<size>-<mtime_epoch>"
            expected_fingerprint = $fingerprint
            current_fingerprint = $currentMarker
        }
    }

    if ($fingerprint -notmatch "^stat:(\d+)-(\d+)$") {
        return [pscustomobject]@{
            path = $path
            current_path = $path
            status = "UNKNOWN"
            reason = "Invalid stat fingerprint pattern"
            expected_fingerprint = $fingerprint
            current_fingerprint = $currentMarker
        }
    }

    $expectedSize = [int64]$matches[1]
    $expectedMtimeEpoch = [int64]$matches[2]
    $sizeMatch = $expectedSize -eq $currentSize
    $mtimeDiff = [Math]::Abs($expectedMtimeEpoch - $currentMtimeEpoch)
    $mtimeMatch = $mtimeDiff -le $ToleranceSeconds

    $status = if ($sizeMatch -and $mtimeMatch) { "MATCH" } else { "MODIFIED" }
    $reason = if ($status -eq "MATCH") {
        "Size and mtime within tolerance"
    } else {
        "Size or mtime changed"
    }

    return [pscustomobject]@{
        path = $path
        current_path = $path
        status = $status
        reason = $reason
        expected_fingerprint = $fingerprint
        current_fingerprint = $currentMarker
    }
}

$resolvedReadme = [System.IO.Path]::GetFullPath($ReadmePath)
if (-not (Test-Path -LiteralPath $resolvedReadme)) {
    throw "README file not found: $resolvedReadme"
}

if (-not $ProjectRoot) {
    $ProjectRoot = Split-Path -Path $resolvedReadme -Parent
}

$resolvedProjectRoot = [System.IO.Path]::GetFullPath($ProjectRoot)
$markdown = Get-Content -LiteralPath $resolvedReadme -Raw
$manifest = Parse-SourceManifestFromMarkdown -Markdown $markdown

if (-not $manifest) {
    $result = [pscustomobject]@{
        readme = $resolvedReadme
        project_root = $resolvedProjectRoot
        manifest_state = "missing"
        update_mode = "FULL"
        reason = "Source Manifest not found in README.AI.md"
        files = @()
    }

    if ($AsJson) {
        $result | ConvertTo-Json -Depth 6
    } else {
        $result
    }
    exit 0
}

$renameMap = Get-RenameMap -RepoRoot $resolvedProjectRoot
$fileStates = @()
foreach ($entry in $manifest.files) {
    if (-not $entry.path) { continue }
    $fileStates += Get-FileStateByStat -Entry $entry -RootPath $resolvedProjectRoot -ToleranceSeconds $MtimeToleranceSeconds -RenameMap $renameMap
}

$counts = @{
    MATCH = 0
    MODIFIED = 0
    DELETED = 0
    RENAMED = 0
    UNKNOWN = 0
}

foreach ($state in $fileStates) {
    if ($counts.ContainsKey($state.status)) {
        $counts[$state.status]++
    } else {
        $counts.UNKNOWN++
    }
}

$updateMode = "FULL"
if ($fileStates.Count -eq 0) {
    $updateMode = "REINDEX"
} elseif ($counts.UNKNOWN -gt 0) {
    $updateMode = "REINDEX"
} elseif ($counts.DELETED -gt 0 -or $counts.RENAMED -gt 0) {
    $updateMode = "FULL"
} elseif ($counts.MODIFIED -gt 0) {
    $updateMode = "INCREMENTAL"
} elseif ($counts.MATCH -eq $fileStates.Count) {
    $updateMode = "SKIP"
}

$summary = [pscustomobject]@{
    readme = $resolvedReadme
    project_root = $resolvedProjectRoot
    manifest_state = "present"
    update_mode = $updateMode
    stat_basis = "size+mtime(epoch-seconds)"
    tolerance_seconds = $MtimeToleranceSeconds
    counts = [pscustomobject]@{
        match = $counts.MATCH
        modified = $counts.MODIFIED
        deleted = $counts.DELETED
        renamed = $counts.RENAMED
        unknown = $counts.UNKNOWN
    }
    files = $fileStates
}

if ($AsJson) {
    $summary | ConvertTo-Json -Depth 8
} else {
    $summary
}
