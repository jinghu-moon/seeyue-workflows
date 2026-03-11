[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$TargetPath,

    [string]$ProjectRoot,

    [string]$ReadmePath,

    [switch]$WriteToReadme,

    [switch]$AsJson,

    [string[]]$IncludeExtensions = @(
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".vue", ".py", ".go", ".java", ".kt",
        ".c", ".h", ".cpp", ".hpp", ".cs", ".json", ".toml", ".yaml", ".yml",
        ".ps1", ".sh", ".bat", ".cmd"
    )
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-FullPath {
    param([string]$PathValue)
    return [System.IO.Path]::GetFullPath($PathValue)
}

function Get-ProjectRoot {
    param(
        [string]$ResolvedTargetPath,
        [string]$UserProjectRoot
    )

    if ($UserProjectRoot) {
        return Resolve-FullPath -PathValue $UserProjectRoot
    }

    if (Test-Path -LiteralPath $ResolvedTargetPath -PathType Container) {
        return $ResolvedTargetPath
    }

    return Split-Path -Path $ResolvedTargetPath -Parent
}

function Get-GitBaseRef {
    param([string]$RootPath)

    try {
        $gitTop = & git -C $RootPath rev-parse --show-toplevel 2>$null
        if ($LASTEXITCODE -eq 0 -and $gitTop) {
            $head = & git -C $RootPath rev-parse --short HEAD 2>$null
            if ($LASTEXITCODE -eq 0 -and $head) {
                return $head.Trim()
            }
            return "working-tree"
        }
    } catch {
        return "working-tree"
    }

    return "working-tree"
}

function Get-RelativePath {
    param(
        [string]$RootPath,
        [string]$AbsolutePath
    )

    $rootUri = [System.Uri]((Resolve-FullPath -PathValue $RootPath).TrimEnd('\') + '\')
    $fileUri = [System.Uri](Resolve-FullPath -PathValue $AbsolutePath)
    $relative = $rootUri.MakeRelativeUri($fileUri).ToString()
    return [System.Uri]::UnescapeDataString($relative).Replace('\', '/')
}

function New-FileFingerprint {
    param([System.IO.FileInfo]$FileItem)

    $size = [int64]$FileItem.Length
    $mtimeEpoch = [int64]([DateTimeOffset]$FileItem.LastWriteTimeUtc).ToUnixTimeSeconds()
    return "stat:$size-$mtimeEpoch"
}

function Should-IncludeFile {
    param(
        [System.IO.FileInfo]$FileItem,
        [string[]]$AllowedExtensions
    )

    if ($FileItem.Name -eq "README.AI.md") {
        return $false
    }

    if ($FileItem.FullName -match "[\\/]\.ai[\\/]") {
        return $false
    }

    $ext = [System.IO.Path]::GetExtension($FileItem.Name)
    return $AllowedExtensions -contains $ext
}

function Build-ManifestObject {
    param(
        [string]$RootPath,
        [System.IO.FileInfo[]]$Files
    )

    $manifestFiles = @()
    foreach ($file in ($Files | Sort-Object FullName)) {
        $manifestFiles += [pscustomobject]@{
            path = Get-RelativePath -RootPath $RootPath -AbsolutePath $file.FullName
            fingerprint = New-FileFingerprint -FileItem $file
        }
    }

    return [pscustomobject]@{
        source_manifest = [pscustomobject]@{
            schema = 1
            generated_at = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
            base_ref = Get-GitBaseRef -RootPath $RootPath
            files = $manifestFiles
        }
    }
}

function Convert-ManifestToYamlBlock {
    param([pscustomobject]$ManifestObject)

    $m = $ManifestObject.source_manifest
    $lines = @()
    $lines += '```yaml'
    $lines += "source_manifest:"
    $lines += "  schema: $($m.schema)"
    $lines += "  generated_at: $($m.generated_at)"
    $lines += "  base_ref: $($m.base_ref)"
    $lines += "  files:"

    foreach ($f in $m.files) {
        $lines += "    - path: $($f.path)"
        $lines += "      fingerprint: $($f.fingerprint)"
    }

    $lines += '```'
    return ($lines -join "`n")
}

function Upsert-SourceManifestSection {
    param(
        [string]$CurrentContent,
        [string]$YamlBlock
    )

    $sectionPattern = '(?s)## Source Manifest\s+```yaml.*?```'
    $newSection = "## Source Manifest`n`n$YamlBlock"

    if ($CurrentContent -match $sectionPattern) {
        return [System.Text.RegularExpressions.Regex]::Replace($CurrentContent, $sectionPattern, $newSection)
    }

    $trimmed = $CurrentContent.TrimEnd()
    return "$trimmed`n`n$newSection`n"
}

$resolvedTargetPath = Resolve-FullPath -PathValue $TargetPath
if (-not (Test-Path -LiteralPath $resolvedTargetPath)) {
    throw "Target path not found: $resolvedTargetPath"
}

$resolvedProjectRoot = Get-ProjectRoot -ResolvedTargetPath $resolvedTargetPath -UserProjectRoot $ProjectRoot
if (-not (Test-Path -LiteralPath $resolvedProjectRoot -PathType Container)) {
    throw "Project root is not a directory: $resolvedProjectRoot"
}

$fileList = @()
if (Test-Path -LiteralPath $resolvedTargetPath -PathType Container) {
    $fileList = Get-ChildItem -Path $resolvedTargetPath -File -Recurse | Where-Object {
        Should-IncludeFile -FileItem $_ -AllowedExtensions $IncludeExtensions
    }
} else {
    $singleFile = Get-Item -LiteralPath $resolvedTargetPath
    if (Should-IncludeFile -FileItem $singleFile -AllowedExtensions $IncludeExtensions) {
        $fileList = @($singleFile)
    }
}

$manifestObject = Build-ManifestObject -RootPath $resolvedProjectRoot -Files $fileList
$yamlBlock = Convert-ManifestToYamlBlock -ManifestObject $manifestObject

if ($WriteToReadme) {
    if (-not $ReadmePath) {
        throw "ReadmePath is required when WriteToReadme is enabled."
    }

    $resolvedReadmePath = Resolve-FullPath -PathValue $ReadmePath
    if (-not (Test-Path -LiteralPath $resolvedReadmePath)) {
        throw "README.AI.md not found: $resolvedReadmePath"
    }

    $current = Get-Content -LiteralPath $resolvedReadmePath -Raw
    $updated = Upsert-SourceManifestSection -CurrentContent $current -YamlBlock $yamlBlock
    Set-Content -LiteralPath $resolvedReadmePath -Value $updated -Encoding utf8NoBOM
}

if ($AsJson) {
    $output = [pscustomobject]@{
        target_path = $resolvedTargetPath
        project_root = $resolvedProjectRoot
        file_count = @($manifestObject.source_manifest.files).Count
        source_manifest = $manifestObject.source_manifest
    }
    $output | ConvertTo-Json -Depth 8
} else {
    $yamlBlock
}
