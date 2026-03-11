[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Task,

    [ValidateSet("SKIP", "INCREMENTAL", "FULL", "REINDEX")]
    [string]$UpdateMode = "INCREMENTAL",

    [int]$ChangedFiles = 0,
    [int]$DeletedFiles = 0,
    [int]$RenamedFiles = 0,

    [switch]$CacheUsed,
    [switch]$ManifestMismatch,

    [ValidateSet("pass", "fail", "skip")]
    [string]$Compile = "skip",
    [ValidateSet("pass", "fail", "skip")]
    [string]$Test = "skip",
    [ValidateSet("pass", "fail", "skip")]
    [string]$Lint = "skip",
    [ValidateSet("pass", "fail", "skip")]
    [string]$Build = "skip",

    [string[]]$ImpactedFile = @(),
    [string[]]$Risk = @(),
    [string[]]$Note = @(),

    [string]$ProjectRoot = ".",
    [string]$OutputDir = ".ai/analysis",
    [string]$ReportBaseName = "ai.report",
    [string]$ReportName = "ai.report",
    [string]$ReportVersion,

    [string]$RunId,
    [string]$PhaseId = "P?",
    [string]$NodeId = "N?",
    [string]$StartedAt,

    [string]$Branch,
    [string]$HeadCommit,
    [string]$BaseRef,
    [switch]$Dirty,

    [string[]]$LanguageStack = @(),
    [switch]$AsJsonOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-FullPath {
    param([string]$PathValue)
    return [System.IO.Path]::GetFullPath($PathValue)
}

function Normalize-RelativePath {
    param([string]$PathValue)
    $v = $PathValue.Replace('\', '/')
    $v = $v.Trim()
    $v = $v.TrimStart('.')
    $v = $v.TrimStart('/')
    return $v
}

function Resolve-AbsoluteFromRoot {
    param(
        [string]$RootPath,
        [string]$PathValue
    )

    if ([System.IO.Path]::IsPathRooted($PathValue)) {
        return [System.IO.Path]::GetFullPath($PathValue)
    }

    return [System.IO.Path]::GetFullPath((Join-Path -Path $RootPath -ChildPath $PathValue))
}

function Get-IsoUtcNow {
    return [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
}

function Parse-IsoOrNow {
    param([string]$Value)

    if (-not $Value) {
        return [DateTimeOffset]::UtcNow
    }

    $dt = [DateTimeOffset]::MinValue
    if ([DateTimeOffset]::TryParse($Value, [ref]$dt)) {
        return $dt.ToUniversalTime()
    }

    return [DateTimeOffset]::UtcNow
}

function Get-PlatformName {
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
        return "windows"
    }
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Linux)) {
        return "linux"
    }
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::OSX)) {
        return "macos"
    }
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

function Get-GitInfo {
    param(
        [string]$RootPath,
        [string]$BranchOverride,
        [string]$HeadOverride,
        [string]$BaseRefOverride,
        [bool]$DirtyOverrideProvided,
        [bool]$DirtyOverrideValue
    )

    $branch = if ($BranchOverride) { $BranchOverride } else { "unknown" }
    $head = if ($HeadOverride) { $HeadOverride } else { "unknown" }
    $baseRef = if ($BaseRefOverride) { $BaseRefOverride } else { "working-tree" }
    $dirty = $DirtyOverrideValue

    try {
        $isGit = & git -C $RootPath rev-parse --is-inside-work-tree 2>$null
        if ($LASTEXITCODE -eq 0 -and ($isGit -join "").Trim() -eq "true") {
            if (-not $BranchOverride) {
                $b = & git -C $RootPath rev-parse --abbrev-ref HEAD 2>$null
                if ($LASTEXITCODE -eq 0 -and $b) { $branch = ($b -join "").Trim() }
            }
            if (-not $HeadOverride) {
                $h = & git -C $RootPath rev-parse HEAD 2>$null
                if ($LASTEXITCODE -eq 0 -and $h) { $head = ($h -join "").Trim() }
            }
            if (-not $BaseRefOverride) {
                $baseRef = if ($head -ne "unknown") { $head } else { "working-tree" }
            }
            if (-not $DirtyOverrideProvided) {
                $status = & git -C $RootPath status --porcelain 2>$null
                $dirty = ($LASTEXITCODE -eq 0) -and ($status) -and (@($status).Count -gt 0)
            }
        } else {
            if (-not $DirtyOverrideProvided) { $dirty = $false }
        }
    } catch {
        if (-not $DirtyOverrideProvided) { $dirty = $false }
    }

    return [pscustomobject]@{
        branch = $branch
        head_commit = $head
        base_ref = $baseRef
        dirty = [bool]$dirty
    }
}

function Get-FileEncodingName {
    param([string]$AbsolutePath)

    try {
        $bytes = [System.IO.File]::ReadAllBytes($AbsolutePath)
        if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) { return "utf-8-bom" }
        if ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFF -and $bytes[1] -eq 0xFE) { return "utf-16-le" }
        if ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFE -and $bytes[1] -eq 0xFF) { return "utf-16-be" }
        if ($bytes.Length -ge 4 -and $bytes[0] -eq 0x00 -and $bytes[1] -eq 0x00 -and $bytes[2] -eq 0xFE -and $bytes[3] -eq 0xFF) { return "utf-32-be" }
        if ($bytes.Length -ge 4 -and $bytes[0] -eq 0xFF -and $bytes[1] -eq 0xFE -and $bytes[2] -eq 0x00 -and $bytes[3] -eq 0x00) { return "utf-32-le" }
        return "utf-8"
    } catch {
        return "unknown"
    }
}

function Get-LineCount {
    param([string]$AbsolutePath)

    try {
        $text = [System.IO.File]::ReadAllText($AbsolutePath)
        if ([string]::IsNullOrEmpty($text)) { return 0 }
        $count = ([regex]::Matches($text, "`r`n|`n|`r")).Count + 1
        return [int]$count
    } catch {
        return 0
    }
}

function Map-StatusToChangeType {
    param(
        [string]$Status,
        [bool]$Exists
    )

    switch ($Status) {
        "MATCH" { return "unchanged" }
        "MODIFIED" { return "modified" }
        "DELETED" { return "deleted" }
        "RENAMED" { return "renamed" }
        default {
            if ($Exists) { return "modified" }
            return "unknown"
        }
    }
}

function Parse-ImpactedFileRecords {
    param(
        [string[]]$Items,
        [string]$RootPath
    )

    $records = @()
    foreach ($raw in $Items) {
        if (-not $raw) { continue }

        # Format (backward compatible):
        # path|role|prob|status|summary|change_type|rename_from|previous_fingerprint|encoding|confidence|blind1;blind2|ev1;ev2|if1;if2|c1;c2|dep1;dep2
        $parts = $raw -split '\|'
        $pathRaw = if ($parts.Count -ge 1) { $parts[0].Trim() } else { "" }
        if (-not $pathRaw) { continue }

        $role = if ($parts.Count -ge 2 -and $parts[1].Trim()) { $parts[1].Trim() } else { "unknown" }
        $prob = if ($parts.Count -ge 3 -and $parts[2].Trim()) { $parts[2].Trim() } else { "Med" }
        $status = if ($parts.Count -ge 4 -and $parts[3].Trim()) { $parts[3].Trim() } else { "UNKNOWN" }
        $summary = if ($parts.Count -ge 5) { $parts[4].Trim() } else { "" }
        $changeType = if ($parts.Count -ge 6 -and $parts[5].Trim()) { $parts[5].Trim().ToLowerInvariant() } else { "" }
        $renameFrom = if ($parts.Count -ge 7 -and $parts[6].Trim()) { Normalize-RelativePath -PathValue $parts[6].Trim() } else { $null }
        $previousFingerprint = if ($parts.Count -ge 8 -and $parts[7].Trim()) { $parts[7].Trim() } else { $null }
        $encodingOverride = if ($parts.Count -ge 9 -and $parts[8].Trim()) { $parts[8].Trim() } else { "" }
        $confidenceRaw = if ($parts.Count -ge 10 -and $parts[9].Trim()) { $parts[9].Trim() } else { "" }
        $blindSpotsRaw = if ($parts.Count -ge 11 -and $parts[10].Trim()) { $parts[10].Trim() } else { "" }
        $evidenceRaw = if ($parts.Count -ge 12 -and $parts[11].Trim()) { $parts[11].Trim() } else { "" }
        $interfacesRaw = if ($parts.Count -ge 13 -and $parts[12].Trim()) { $parts[12].Trim() } else { "" }
        $constraintsRaw = if ($parts.Count -ge 14 -and $parts[13].Trim()) { $parts[13].Trim() } else { "" }
        $dependenciesRaw = if ($parts.Count -ge 15 -and $parts[14].Trim()) { $parts[14].Trim() } else { "" }

        if ($prob -notin @("High", "Med", "Low")) { $prob = "Med" }
        if ($status -notin @("MATCH", "MODIFIED", "DELETED", "RENAMED", "UNKNOWN")) { $status = "UNKNOWN" }

        $relativePath = Normalize-RelativePath -PathValue $pathRaw
        $absolutePath = Resolve-AbsoluteFromRoot -RootPath $RootPath -PathValue $relativePath
        $name = [System.IO.Path]::GetFileName($relativePath)

        $exists = Test-Path -LiteralPath $absolutePath -PathType Leaf
        $sizeBytes = 0
        $usedBytes = 0
        $lineCount = 0
        $encoding = "unknown"
        $lastModified = Get-IsoUtcNow
        $lastModifiedEpoch = 0
        $fingerprint = "stat:0-0"

        if ($exists) {
            $item = Get-Item -LiteralPath $absolutePath
            $sizeBytes = [int64]$item.Length
            $usedBytes = [int64]$item.Length
            $lineCount = Get-LineCount -AbsolutePath $absolutePath
            $encoding = if ($encodingOverride) { $encodingOverride } else { Get-FileEncodingName -AbsolutePath $absolutePath }
            $lastModified = ([DateTimeOffset]$item.LastWriteTimeUtc).ToString("yyyy-MM-ddTHH:mm:ssZ")
            $lastModifiedEpoch = [int64]([DateTimeOffset]$item.LastWriteTimeUtc).ToUnixTimeSeconds()
            $fingerprint = "stat:$sizeBytes-$lastModifiedEpoch"
            if ($status -eq "UNKNOWN") { $status = "MODIFIED" }
        } else {
            if (-not $encodingOverride) { $encoding = "unknown" } else { $encoding = $encodingOverride }
            if ($status -eq "UNKNOWN") { $status = "DELETED" }
        }

        if (-not $changeType) {
            $changeType = Map-StatusToChangeType -Status $status -Exists $exists
        } elseif ($changeType -notin @("added", "modified", "deleted", "renamed", "unchanged", "unknown")) {
            $changeType = Map-StatusToChangeType -Status $status -Exists $exists
        }

        $confidence = 0.6
        if (-not $summary) { $confidence = 0.3 }
        if ($confidenceRaw) {
            $tmp = 0.0
            if ([double]::TryParse($confidenceRaw, [ref]$tmp)) {
                $confidence = $tmp
            }
        }
        if ($confidence -lt 0) { $confidence = 0.0 }
        if ($confidence -gt 1) { $confidence = 1.0 }

        $blindSpots = @()
        if ($blindSpotsRaw) {
            $blindSpots = @(
                $blindSpotsRaw -split ';' |
                ForEach-Object { $_.Trim() } |
                Where-Object {
                    $_ -and $_.ToLowerInvariant() -notin @("none", "-", "null", "n/a")
                }
            )
        }

        $evidence = @()
        if ($evidenceRaw) {
            $evidence = @(
                $evidenceRaw -split ';' |
                ForEach-Object { $_.Trim() } |
                Where-Object {
                    $_ -and $_.ToLowerInvariant() -notin @("none", "-", "null", "n/a")
                }
            )
        }

        $interfaces = @()
        if ($interfacesRaw) {
            $interfaces = @(
                $interfacesRaw -split ';' |
                ForEach-Object { $_.Trim() } |
                Where-Object {
                    $_ -and $_.ToLowerInvariant() -notin @("none", "-", "null", "n/a")
                }
            )
        }

        $constraints = @()
        if ($constraintsRaw) {
            $constraints = @(
                $constraintsRaw -split ';' |
                ForEach-Object { $_.Trim() } |
                Where-Object {
                    $_ -and $_.ToLowerInvariant() -notin @("none", "-", "null", "n/a")
                }
            )
        }

        $dependencies = @()
        if ($dependenciesRaw) {
            $dependencies = @(
                $dependenciesRaw -split ';' |
                ForEach-Object { $_.Trim() } |
                Where-Object {
                    $_ -and $_.ToLowerInvariant() -notin @("none", "-", "null", "n/a")
                }
            )
        }

        $record = [pscustomobject]@{
            path = $relativePath
            name = $name
            exists = [bool]$exists
            size_bytes = [int64]$sizeBytes
            used_bytes = [int64]$usedBytes
            line_count = [int]$lineCount
            encoding = $encoding
            last_modified = $lastModified
            last_modified_epoch = [int64]$lastModifiedEpoch
            fingerprint = $fingerprint
            previous_fingerprint = $previousFingerprint
            status = $status
            change_type = $changeType
            rename_from = $renameFrom
            role = $role
            modify_prob = $prob
            understanding = [pscustomobject]@{
                summary = $summary
                interfaces = $interfaces
                constraints = $constraints
                dependencies = $dependencies
                evidence = $evidence
                confidence = [double]$confidence
                blind_spots = $blindSpots
                evidence_count = [int]$evidence.Count
            }
        }

        $records += $record
    }

    $dedup = @{}
    foreach ($record in $records) {
        $dedup[$record.path] = $record
    }

    return @($dedup.Values | Sort-Object path)
}

function New-DirectoryNode {
    param(
        [string]$Name,
        [string]$PathValue
    )

    return [ordered]@{
        type = "directory"
        name = $Name
        path = $PathValue
        children = [System.Collections.ArrayList]::new()
    }
}

function New-FileNode {
    param([pscustomobject]$Record)

    return [ordered]@{
        type = "file"
        name = $Record.name
        path = $Record.path
        exists = [bool]$Record.exists
        size_bytes = [int64]$Record.size_bytes
        used_bytes = [int64]$Record.used_bytes
        line_count = [int]$Record.line_count
        encoding = $Record.encoding
        last_modified = $Record.last_modified
        last_modified_epoch = [int64]$Record.last_modified_epoch
        fingerprint = $Record.fingerprint
        previous_fingerprint = $Record.previous_fingerprint
        status = $Record.status
        change_type = $Record.change_type
        rename_from = $Record.rename_from
    }
}

function Get-OrAddDirectoryChild {
    param(
        [hashtable]$ParentNode,
        [string]$ChildName,
        [string]$ChildPath
    )

    foreach ($child in $ParentNode.children) {
        if (($child.type -eq "directory") -and ($child.name -eq $ChildName)) {
            return $child
        }
    }

    $node = New-DirectoryNode -Name $ChildName -PathValue $ChildPath
    [void]$ParentNode.children.Add($node)
    return $node
}

function Add-RecordToTree {
    param(
        [hashtable]$RootNode,
        [pscustomobject]$Record
    )

    $segments = $Record.path -split '/'
    if ($segments.Count -eq 0) { return }

    $cursor = $RootNode
    $dirCount = $segments.Count - 1
    for ($i = 0; $i -lt $dirCount; $i++) {
        $segment = $segments[$i]
        if (-not $segment) { continue }
        $subPath = ($segments[0..$i] -join '/')
        $cursor = Get-OrAddDirectoryChild -ParentNode $cursor -ChildName $segment -ChildPath $subPath
    }

    [void]$cursor.children.Add((New-FileNode -Record $Record))
}

function Sort-TreeChildren {
    param([hashtable]$Node)

    $dirs = @()
    $files = @()
    foreach ($child in $Node.children) {
        if ($child.type -eq "directory") {
            Sort-TreeChildren -Node $child
            $dirs += $child
        } else {
            $files += $child
        }
    }

    $sorted = @($dirs | Sort-Object name) + @($files | Sort-Object name)
    $Node.children.Clear()
    foreach ($item in $sorted) { [void]$Node.children.Add($item) }
}

function Build-ReportTree {
    param([pscustomobject[]]$Records)

    $root = New-DirectoryNode -Name "root" -PathValue "."
    foreach ($r in $Records) {
        Add-RecordToTree -RootNode $root -Record $r
    }
    Sort-TreeChildren -Node $root
    return [pscustomobject]$root
}

function Convert-ReportToMarkdown {
    param(
        [pscustomobject]$Report,
        [string]$JsonPath
    )

    $lines = @()
    $lines += "# AI Report"
    $lines += ""
    $lines += "- Report Name: $($Report.report_name)"
    $lines += "- Report Version: $($Report.report_version)"
    $lines += "- Schema: $($Report.schema_name) v$($Report.schema_version)"
    $lines += "- Generated At: $($Report.generated_at)"
    $lines += "- Updated At: $($Report.updated_at)"
    $lines += "- Task: $($Report.task)"
    $lines += "- Update Mode: $($Report.update_mode)"
    $lines += ""
    $lines += "## Project"
    $lines += ""
    $lines += "- name: $($Report.project.name)"
    $lines += "- root: $($Report.project.root)"
    $lines += "- platform: $($Report.project.platform)"
    $lines += "- language_stack: $([string]::Join(', ', $Report.project.language_stack))"
    $lines += ""
    $lines += "## SCM"
    $lines += ""
    $lines += "- branch: $($Report.scm.branch)"
    $lines += "- head_commit: $($Report.scm.head_commit)"
    $lines += "- base_ref: $($Report.scm.base_ref)"
    $lines += "- dirty: $($Report.scm.dirty)"
    $lines += ""
    $lines += "## Run"
    $lines += ""
    $lines += "- run_id: $($Report.run.run_id)"
    $lines += "- phase_id: $($Report.run.phase_id)"
    $lines += "- node_id: $($Report.run.node_id)"
    $lines += "- started_at: $($Report.run.started_at)"
    $lines += "- finished_at: $($Report.run.finished_at)"
    $lines += "- duration_ms: $($Report.run.duration_ms)"
    $lines += ""
    $lines += "## File Summary"
    $lines += ""
    if (@($Report.files).Count -eq 0) {
        $lines += "- None"
    } else {
        foreach ($f in @($Report.files)) {
            $lines += "- $($f.path) | change=$($f.change_type) | status=$($f.status) | bytes=$($f.size_bytes)"
        }
    }
    $lines += ""
    $lines += "Machine-readable source: $JsonPath"
    return ($lines -join "`n")
}

$resolvedProjectRoot = Resolve-FullPath -PathValue $ProjectRoot
if (-not (Test-Path -LiteralPath $resolvedProjectRoot -PathType Container)) {
    throw "ProjectRoot is not a directory: $resolvedProjectRoot"
}

$resolvedOutputDir = Resolve-FullPath -PathValue $OutputDir
if (-not (Test-Path -LiteralPath $resolvedOutputDir)) {
    New-Item -ItemType Directory -Path $resolvedOutputDir -Force | Out-Null
}

if (-not $ReportVersion) {
    $ReportVersion = [DateTime]::UtcNow.ToString("yyyy.MM.dd.HHmmss")
}
if (-not $RunId) {
    $RunId = [guid]::NewGuid().ToString("N")
}

$start = Parse-IsoOrNow -Value $StartedAt
$finish = [DateTimeOffset]::UtcNow
$durationMs = [int64]([math]::Max(0, ($finish - $start).TotalMilliseconds))

$effectiveLanguageStack = if ($LanguageStack -and $LanguageStack.Count -gt 0) { $LanguageStack } else { Detect-LanguageStack -RootPath $resolvedProjectRoot }

$gitInfo = Get-GitInfo `
    -RootPath $resolvedProjectRoot `
    -BranchOverride $Branch `
    -HeadOverride $HeadCommit `
    -BaseRefOverride $BaseRef `
    -DirtyOverrideProvided ([bool]$PSBoundParameters.ContainsKey("Dirty")) `
    -DirtyOverrideValue ([bool]$Dirty.IsPresent)

$records = @(Parse-ImpactedFileRecords -Items $ImpactedFile -RootPath $resolvedProjectRoot)
$tree = Build-ReportTree -Records $records
$now = Get-IsoUtcNow

$report = [pscustomobject]@{
    schema_name = "ai.report"
    schema_version = 3
    report_name = $ReportName
    report_version = $ReportVersion
    generated_at = $now
    updated_at = $now
    task = $Task
    project = [pscustomobject]@{
        name = [System.IO.Path]::GetFileName($resolvedProjectRoot)
        root = $resolvedProjectRoot.Replace('\', '/')
        platform = Get-PlatformName
        language_stack = $effectiveLanguageStack
    }
    scm = $gitInfo
    run = [pscustomobject]@{
        run_id = $RunId
        phase_id = $PhaseId
        node_id = $NodeId
        started_at = $start.ToString("yyyy-MM-ddTHH:mm:ssZ")
        finished_at = $finish.ToString("yyyy-MM-ddTHH:mm:ssZ")
        duration_ms = $durationMs
    }
    update_mode = $UpdateMode
    delta_basis = [pscustomobject]@{
        changed_files = $ChangedFiles
        deleted_files = $DeletedFiles
        renamed_files = $RenamedFiles
        cache_used = [bool]$CacheUsed.IsPresent
        manifest_mismatch = [bool]$ManifestMismatch.IsPresent
    }
    verification = [pscustomobject]@{
        compile = $Compile
        test = $Test
        lint = $Lint
        build = $Build
    }
    risks = $Risk
    notes = $Note
    files = @($records)
    tree = $tree
}

$jsonPath = Join-Path -Path $resolvedOutputDir -ChildPath "$ReportBaseName.json"
$mdPath = Join-Path -Path $resolvedOutputDir -ChildPath "$ReportBaseName.md"

$jsonContent = $report | ConvertTo-Json -Depth 30
Set-Content -LiteralPath $jsonPath -Value $jsonContent -Encoding utf8NoBOM

if (-not $AsJsonOnly) {
    $mdContent = Convert-ReportToMarkdown -Report $report -JsonPath $jsonPath
    Set-Content -LiteralPath $mdPath -Value $mdContent -Encoding utf8NoBOM
}

$mdOutputPath = $mdPath
if ($AsJsonOnly.IsPresent) {
    $mdOutputPath = $null
}

[pscustomobject]@{
    json_path = $jsonPath
    md_path = $mdOutputPath
    schema_version = 3
    report_name = $ReportName
    report_version = $ReportVersion
    update_mode = $UpdateMode
    files_count = @($records).Count
    run_id = $RunId
} | ConvertTo-Json -Depth 8
