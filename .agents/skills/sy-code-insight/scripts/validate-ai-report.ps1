[CmdletBinding()]
param(
    [string]$ReportPath = ".ai/analysis/ai.report.json",
    [string]$SchemaPath = ".agents/skills/sy-code-insight/references/ai-report.schema.json",
    [switch]$AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-FullPath {
    param([string]$PathValue)
    return [System.IO.Path]::GetFullPath($PathValue)
}

function Add-ValidationError {
    param(
        [ref]$Errors,
        [string]$Path,
        [string]$Message
    )

    $Errors.Value += [pscustomobject]@{
        path = $Path
        message = $Message
    }
}

function Is-IntegerLike {
    param([object]$Value)
    return ($Value -is [int]) -or ($Value -is [long]) -or ($Value -is [int64])
}

function Is-NumberLike {
    param([object]$Value)
    return ($Value -is [double]) -or ($Value -is [single]) -or ($Value -is [decimal]) -or (Is-IntegerLike -Value $Value)
}

function Is-DateTimeLike {
    param([object]$Value)
    if ($Value -is [DateTime] -or $Value -is [DateTimeOffset]) {
        return $true
    }
    if ($Value -isnot [string] -or [string]::IsNullOrWhiteSpace($Value)) {
        return $false
    }
    $dto = [DateTimeOffset]::MinValue
    return [DateTimeOffset]::TryParse([string]$Value, [ref]$dto)
}

function Is-NonEmptyString {
    param([object]$Value)
    return ($Value -is [string]) -and (-not [string]::IsNullOrWhiteSpace($Value))
}

function Validate-StringArray {
    param(
        [object]$Value,
        [string]$Context,
        [ref]$Errors
    )

    if ($Value -isnot [array]) {
        Add-ValidationError -Errors $Errors -Path $Context -Message "Must be array"
        return
    }

    for ($i = 0; $i -lt $Value.Count; $i++) {
        if ($Value[$i] -isnot [string]) {
            Add-ValidationError -Errors $Errors -Path "$Context[$i]" -Message "Must be string"
        }
    }
}

function Validate-Understanding {
    param(
        [hashtable]$Value,
        [string]$Context,
        [ref]$Errors
    )

    $required = @("summary", "interfaces", "constraints", "dependencies", "evidence", "confidence", "blind_spots", "evidence_count")
    foreach ($k in $required) {
        if (-not $Value.ContainsKey($k)) {
            Add-ValidationError -Errors $Errors -Path $Context -Message "Missing required property: $k"
        }
    }

    if ($Value.ContainsKey("summary") -and $Value.summary -isnot [string]) {
        Add-ValidationError -Errors $Errors -Path "$Context.summary" -Message "Must be string"
    }

    foreach ($arr in @("interfaces", "constraints", "dependencies", "evidence", "blind_spots")) {
        if ($Value.ContainsKey($arr)) {
            Validate-StringArray -Value $Value[$arr] -Context "$Context.$arr" -Errors $Errors
        }
    }

    if ($Value.ContainsKey("confidence")) {
        if (-not (Is-NumberLike -Value $Value.confidence)) {
            Add-ValidationError -Errors $Errors -Path "$Context.confidence" -Message "Must be number in [0,1]"
        } else {
            $c = [double]$Value.confidence
            if ($c -lt 0 -or $c -gt 1) {
                Add-ValidationError -Errors $Errors -Path "$Context.confidence" -Message "Must be between 0 and 1"
            }
        }
    }

    if ($Value.ContainsKey("evidence_count")) {
        if (-not (Is-IntegerLike -Value $Value.evidence_count)) {
            Add-ValidationError -Errors $Errors -Path "$Context.evidence_count" -Message "Must be integer >= 0"
        } elseif ([int64]$Value.evidence_count -lt 0) {
            Add-ValidationError -Errors $Errors -Path "$Context.evidence_count" -Message "Must be >= 0"
        } elseif ($Value.ContainsKey("evidence") -and ($Value.evidence -is [array])) {
            if ([int64]$Value.evidence_count -ne $Value.evidence.Count) {
                Add-ValidationError -Errors $Errors -Path "$Context.evidence_count" -Message "Must equal evidence array length"
            }
        }
    }

    $evidenceArr = @()
    if ($Value.ContainsKey("evidence") -and ($Value.evidence -is [array])) {
        $evidenceArr = @($Value.evidence)
    }
    $blindSpotsArr = @()
    if ($Value.ContainsKey("blind_spots") -and ($Value.blind_spots -is [array])) {
        $blindSpotsArr = @($Value.blind_spots)
    }

    # Rule 3: blind_spots non-empty -> confidence MUST NOT exceed 0.7
    if (($blindSpotsArr.Count -gt 0) -and $Value.ContainsKey("confidence") -and (Is-NumberLike -Value $Value.confidence)) {
        if ([double]$Value.confidence -gt 0.7) {
            Add-ValidationError -Errors $Errors -Path "$Context.confidence" -Message "Must be <= 0.7 when blind_spots is non-empty"
        }
    }

    # Rule 4: when no evidence exists, confidence must be low and blind_spots must be declared
    if ($evidenceArr.Count -lt 1) {
        if ($Value.ContainsKey("confidence") -and (Is-NumberLike -Value $Value.confidence)) {
            if ([double]$Value.confidence -gt 0.5) {
                Add-ValidationError -Errors $Errors -Path "$Context.confidence" -Message "Must be <= 0.5 when evidence is empty (low-confidence required)"
            }
        }
        if ($blindSpotsArr.Count -lt 1) {
            Add-ValidationError -Errors $Errors -Path "$Context.blind_spots" -Message "Must include at least one blind_spot when evidence is empty"
        }
    }
}

function Validate-FileLikeRecord {
    param(
        [hashtable]$Value,
        [string]$Context,
        [bool]$WithUnderstanding,
        [ref]$Errors
    )

    $required = @(
        "path", "name", "exists", "size_bytes", "used_bytes", "line_count",
        "encoding", "last_modified", "last_modified_epoch",
        "fingerprint", "previous_fingerprint", "status", "change_type", "rename_from"
    )
    if ($WithUnderstanding) {
        $required += @("role", "modify_prob", "understanding")
    }

    foreach ($k in $required) {
        if (-not $Value.ContainsKey($k)) {
            Add-ValidationError -Errors $Errors -Path $Context -Message "Missing required property: $k"
        }
    }

    foreach ($s in @("path", "name", "encoding", "fingerprint")) {
        if ($Value.ContainsKey($s) -and -not (Is-NonEmptyString -Value $Value[$s])) {
            Add-ValidationError -Errors $Errors -Path "$Context.$s" -Message "Must be non-empty string"
        }
    }

    if ($Value.ContainsKey("exists") -and $Value.exists -isnot [bool]) {
        Add-ValidationError -Errors $Errors -Path "$Context.exists" -Message "Must be boolean"
    }

    foreach ($n in @("size_bytes", "used_bytes", "line_count", "last_modified_epoch")) {
        if ($Value.ContainsKey($n)) {
            if (-not (Is-IntegerLike -Value $Value[$n])) {
                Add-ValidationError -Errors $Errors -Path "$Context.$n" -Message "Must be integer >= 0"
            } elseif ([int64]$Value[$n] -lt 0) {
                Add-ValidationError -Errors $Errors -Path "$Context.$n" -Message "Must be >= 0"
            }
        }
    }

    if ($Value.ContainsKey("last_modified") -and -not (Is-DateTimeLike -Value $Value.last_modified)) {
        Add-ValidationError -Errors $Errors -Path "$Context.last_modified" -Message "Must be valid date-time"
    }

    if ($Value.ContainsKey("previous_fingerprint")) {
        if (($Value.previous_fingerprint -ne $null) -and ($Value.previous_fingerprint -isnot [string])) {
            Add-ValidationError -Errors $Errors -Path "$Context.previous_fingerprint" -Message "Must be string or null"
        }
    }

    if ($Value.ContainsKey("rename_from")) {
        if (($Value.rename_from -ne $null) -and ($Value.rename_from -isnot [string])) {
            Add-ValidationError -Errors $Errors -Path "$Context.rename_from" -Message "Must be string or null"
        }
    }

    if ($Value.ContainsKey("status") -and $Value.status -notin @("MATCH", "MODIFIED", "DELETED", "RENAMED", "UNKNOWN")) {
        Add-ValidationError -Errors $Errors -Path "$Context.status" -Message "Must be one of MATCH|MODIFIED|DELETED|RENAMED|UNKNOWN"
    }

    if ($Value.ContainsKey("change_type") -and $Value.change_type -notin @("added", "modified", "deleted", "renamed", "unchanged", "unknown")) {
        Add-ValidationError -Errors $Errors -Path "$Context.change_type" -Message "Must be one of added|modified|deleted|renamed|unchanged|unknown"
    }

    if ($WithUnderstanding) {
        if ($Value.ContainsKey("role") -and -not (Is-NonEmptyString -Value $Value.role)) {
            Add-ValidationError -Errors $Errors -Path "$Context.role" -Message "Must be non-empty string"
        }
        if ($Value.ContainsKey("modify_prob") -and $Value.modify_prob -notin @("High", "Med", "Low")) {
            Add-ValidationError -Errors $Errors -Path "$Context.modify_prob" -Message "Must be one of High|Med|Low"
        }

        if ($Value.ContainsKey("understanding")) {
            if ($Value.understanding -isnot [hashtable]) {
                Add-ValidationError -Errors $Errors -Path "$Context.understanding" -Message "Must be object"
            } else {
                Validate-Understanding -Value $Value.understanding -Context "$Context.understanding" -Errors $Errors

                # Rule 5: High-impact files require non-empty interfaces/constraints/evidence
                if ($Value.ContainsKey("modify_prob") -and $Value.modify_prob -eq "High") {
                    $interfaces = @()
                    if ($Value.understanding.ContainsKey("interfaces") -and ($Value.understanding.interfaces -is [array])) {
                        $interfaces = @($Value.understanding.interfaces)
                    }
                    $constraints = @()
                    if ($Value.understanding.ContainsKey("constraints") -and ($Value.understanding.constraints -is [array])) {
                        $constraints = @($Value.understanding.constraints)
                    }
                    $evidence = @()
                    if ($Value.understanding.ContainsKey("evidence") -and ($Value.understanding.evidence -is [array])) {
                        $evidence = @($Value.understanding.evidence)
                    }

                    if ($interfaces.Count -lt 1) {
                        Add-ValidationError -Errors $Errors -Path "$Context.understanding.interfaces" -Message "High-impact file must include at least one interface"
                    }
                    if ($constraints.Count -lt 1) {
                        Add-ValidationError -Errors $Errors -Path "$Context.understanding.constraints" -Message "High-impact file must include at least one constraint"
                    }
                    if ($evidence.Count -lt 1) {
                        Add-ValidationError -Errors $Errors -Path "$Context.understanding.evidence" -Message "High-impact file must include at least one evidence"
                    }
                }
            }
        }
    }
}

function Validate-TreeNode {
    param(
        [hashtable]$Node,
        [string]$Context,
        [ref]$Errors
    )

    foreach ($k in @("type", "name", "path")) {
        if (-not $Node.ContainsKey($k)) {
            Add-ValidationError -Errors $Errors -Path $Context -Message "Missing required property: $k"
        }
    }

    if ($Node.ContainsKey("type") -and $Node.type -notin @("directory", "file")) {
        Add-ValidationError -Errors $Errors -Path "$Context.type" -Message "Must be directory or file"
        return
    }

    if ($Node.ContainsKey("name") -and -not (Is-NonEmptyString -Value $Node.name)) {
        Add-ValidationError -Errors $Errors -Path "$Context.name" -Message "Must be non-empty string"
    }
    if ($Node.ContainsKey("path") -and -not (Is-NonEmptyString -Value $Node.path)) {
        Add-ValidationError -Errors $Errors -Path "$Context.path" -Message "Must be non-empty string"
    }

    if ($Node.type -eq "directory") {
        if (-not $Node.ContainsKey("children")) {
            Add-ValidationError -Errors $Errors -Path $Context -Message "Directory must contain children"
            return
        }
        if ($Node.children -isnot [array]) {
            Add-ValidationError -Errors $Errors -Path "$Context.children" -Message "Must be array"
            return
        }
        for ($i = 0; $i -lt $Node.children.Count; $i++) {
            if ($Node.children[$i] -isnot [hashtable]) {
                Add-ValidationError -Errors $Errors -Path "$Context.children[$i]" -Message "Must be object"
                continue
            }
            Validate-TreeNode -Node $Node.children[$i] -Context "$Context.children[$i]" -Errors $Errors
        }
    } else {
        Validate-FileLikeRecord -Value $Node -Context $Context -WithUnderstanding:$false -Errors $Errors
    }
}

function Collect-TreeFilePaths {
    param([hashtable]$Node)

    if ($Node.type -eq "file") {
        return @([string]$Node.path)
    }

    $result = @()
    if ($Node.type -eq "directory" -and $Node.ContainsKey("children") -and $Node.children -is [array]) {
        foreach ($child in $Node.children) {
            if ($child -is [hashtable]) {
                $result += Collect-TreeFilePaths -Node $child
            }
        }
    }
    return $result
}

$resolvedReportPath = Resolve-FullPath -PathValue $ReportPath
$resolvedSchemaPath = Resolve-FullPath -PathValue $SchemaPath
$errors = @()

if (-not (Test-Path -LiteralPath $resolvedSchemaPath)) {
    Add-ValidationError -Errors ([ref]$errors) -Path "schema" -Message "Schema file not found: $resolvedSchemaPath"
}

$report = $null
if (-not (Test-Path -LiteralPath $resolvedReportPath)) {
    Add-ValidationError -Errors ([ref]$errors) -Path "report" -Message "Report file not found: $resolvedReportPath"
} else {
    try {
        $text = Get-Content -LiteralPath $resolvedReportPath -Raw
        $report = ConvertFrom-Json -InputObject $text -AsHashtable
    } catch {
        Add-ValidationError -Errors ([ref]$errors) -Path "report" -Message "Invalid JSON: $($_.Exception.Message)"
    }
}

if ($report -is [hashtable]) {
    $requiredTop = @(
        "schema_name", "schema_version", "report_name", "report_version",
        "generated_at", "updated_at", "task", "project", "scm", "run",
        "update_mode", "delta_basis", "verification", "risks", "notes", "files", "tree"
    )
    foreach ($k in $requiredTop) {
        if (-not $report.ContainsKey($k)) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$" -Message "Missing required property: $k"
        }
    }

    if ($report.ContainsKey("schema_name") -and $report.schema_name -ne "ai.report") {
        Add-ValidationError -Errors ([ref]$errors) -Path "$.schema_name" -Message "Must equal ai.report"
    }
    if ($report.ContainsKey("schema_version")) {
        if (-not (Is-IntegerLike -Value $report.schema_version)) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.schema_version" -Message "Must be integer"
        } elseif ([int64]$report.schema_version -ne 3) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.schema_version" -Message "Must equal 3"
        }
    }

    foreach ($k in @("report_name", "report_version", "task")) {
        if ($report.ContainsKey($k) -and -not (Is-NonEmptyString -Value $report[$k])) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.$k" -Message "Must be non-empty string"
        }
    }
    foreach ($k in @("generated_at", "updated_at")) {
        if ($report.ContainsKey($k) -and -not (Is-DateTimeLike -Value $report[$k])) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.$k" -Message "Must be valid date-time"
        }
    }
    if ($report.ContainsKey("update_mode") -and $report.update_mode -notin @("SKIP", "INCREMENTAL", "FULL", "REINDEX")) {
        Add-ValidationError -Errors ([ref]$errors) -Path "$.update_mode" -Message "Must be one of SKIP|INCREMENTAL|FULL|REINDEX"
    }

    if ($report.ContainsKey("project")) {
        if ($report.project -isnot [hashtable]) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.project" -Message "Must be object"
        } else {
            foreach ($k in @("name", "root", "platform", "language_stack")) {
                if (-not $report.project.ContainsKey($k)) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.project" -Message "Missing required property: $k"
                }
            }
            foreach ($k in @("name", "root", "platform")) {
                if ($report.project.ContainsKey($k) -and -not (Is-NonEmptyString -Value $report.project[$k])) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.project.$k" -Message "Must be non-empty string"
                }
            }
            if ($report.project.ContainsKey("language_stack")) {
                Validate-StringArray -Value $report.project.language_stack -Context "$.project.language_stack" -Errors ([ref]$errors)
            }
        }
    }

    if ($report.ContainsKey("scm")) {
        if ($report.scm -isnot [hashtable]) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.scm" -Message "Must be object"
        } else {
            foreach ($k in @("branch", "head_commit", "base_ref", "dirty")) {
                if (-not $report.scm.ContainsKey($k)) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.scm" -Message "Missing required property: $k"
                }
            }
            foreach ($k in @("branch", "head_commit", "base_ref")) {
                if ($report.scm.ContainsKey($k) -and ($report.scm[$k] -isnot [string])) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.scm.$k" -Message "Must be string"
                }
            }
            if ($report.scm.ContainsKey("dirty") -and $report.scm.dirty -isnot [bool]) {
                Add-ValidationError -Errors ([ref]$errors) -Path "$.scm.dirty" -Message "Must be boolean"
            }
        }
    }

    if ($report.ContainsKey("run")) {
        if ($report.run -isnot [hashtable]) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.run" -Message "Must be object"
        } else {
            foreach ($k in @("run_id", "phase_id", "node_id", "started_at", "finished_at", "duration_ms")) {
                if (-not $report.run.ContainsKey($k)) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.run" -Message "Missing required property: $k"
                }
            }
            foreach ($k in @("run_id", "phase_id", "node_id")) {
                if ($report.run.ContainsKey($k) -and -not (Is-NonEmptyString -Value $report.run[$k])) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.run.$k" -Message "Must be non-empty string"
                }
            }
            foreach ($k in @("started_at", "finished_at")) {
                if ($report.run.ContainsKey($k) -and -not (Is-DateTimeLike -Value $report.run[$k])) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.run.$k" -Message "Must be valid date-time"
                }
            }
            if ($report.run.ContainsKey("duration_ms")) {
                if (-not (Is-IntegerLike -Value $report.run.duration_ms)) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.run.duration_ms" -Message "Must be integer >= 0"
                } elseif ([int64]$report.run.duration_ms -lt 0) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.run.duration_ms" -Message "Must be >= 0"
                }
            }
        }
    }

    if ($report.ContainsKey("delta_basis")) {
        if ($report.delta_basis -isnot [hashtable]) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.delta_basis" -Message "Must be object"
        } else {
            foreach ($k in @("changed_files", "deleted_files", "renamed_files", "cache_used", "manifest_mismatch")) {
                if (-not $report.delta_basis.ContainsKey($k)) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.delta_basis" -Message "Missing required property: $k"
                }
            }
            foreach ($k in @("changed_files", "deleted_files", "renamed_files")) {
                if ($report.delta_basis.ContainsKey($k)) {
                    if (-not (Is-IntegerLike -Value $report.delta_basis[$k])) {
                        Add-ValidationError -Errors ([ref]$errors) -Path "$.delta_basis.$k" -Message "Must be integer >= 0"
                    } elseif ([int64]$report.delta_basis[$k] -lt 0) {
                        Add-ValidationError -Errors ([ref]$errors) -Path "$.delta_basis.$k" -Message "Must be >= 0"
                    }
                }
            }
            foreach ($k in @("cache_used", "manifest_mismatch")) {
                if ($report.delta_basis.ContainsKey($k) -and $report.delta_basis[$k] -isnot [bool]) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.delta_basis.$k" -Message "Must be boolean"
                }
            }
        }
    }

    if ($report.ContainsKey("verification")) {
        if ($report.verification -isnot [hashtable]) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.verification" -Message "Must be object"
        } else {
            foreach ($k in @("compile", "test", "lint", "build")) {
                if (-not $report.verification.ContainsKey($k)) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.verification" -Message "Missing required property: $k"
                } elseif ($report.verification[$k] -notin @("pass", "fail", "skip")) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.verification.$k" -Message "Must be one of pass|fail|skip"
                }
            }
        }
    }

    if ($report.ContainsKey("risks")) { Validate-StringArray -Value $report.risks -Context "$.risks" -Errors ([ref]$errors) }
    if ($report.ContainsKey("notes")) { Validate-StringArray -Value $report.notes -Context "$.notes" -Errors ([ref]$errors) }

    if ($report.ContainsKey("files")) {
        if ($report.files -isnot [array]) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.files" -Message "Must be array"
        } else {
            for ($i = 0; $i -lt $report.files.Count; $i++) {
                if ($report.files[$i] -isnot [hashtable]) {
                    Add-ValidationError -Errors ([ref]$errors) -Path "$.files[$i]" -Message "Must be object"
                    continue
                }
                Validate-FileLikeRecord -Value $report.files[$i] -Context "$.files[$i]" -WithUnderstanding:$true -Errors ([ref]$errors)
            }
        }
    }

    if ($report.ContainsKey("tree")) {
        if ($report.tree -isnot [hashtable]) {
            Add-ValidationError -Errors ([ref]$errors) -Path "$.tree" -Message "Must be object"
        } else {
            Validate-TreeNode -Node $report.tree -Context "$.tree" -Errors ([ref]$errors)
        }
    }

    if (($report.ContainsKey("files")) -and ($report.files -is [array]) -and ($report.ContainsKey("tree")) -and ($report.tree -is [hashtable])) {
        $flatPaths = @{}
        foreach ($f in $report.files) {
            if ($f -is [hashtable] -and $f.ContainsKey("path")) {
                $flatPaths[[string]$f.path] = $true
            }
        }

        $treePaths = @{}
        foreach ($p in (Collect-TreeFilePaths -Node $report.tree)) {
            $treePaths[[string]$p] = $true
        }

        foreach ($p in $flatPaths.Keys) {
            if (-not $treePaths.ContainsKey($p)) {
                Add-ValidationError -Errors ([ref]$errors) -Path "$.tree" -Message "Missing file path from tree: $p"
            }
        }
        foreach ($p in $treePaths.Keys) {
            if (-not $flatPaths.ContainsKey($p)) {
                Add-ValidationError -Errors ([ref]$errors) -Path "$.files" -Message "Missing file record for tree node: $p"
            }
        }
    }
}

$result = [pscustomobject]@{
    report_path = $resolvedReportPath
    schema_path = $resolvedSchemaPath
    valid = ($errors.Count -eq 0)
    error_count = $errors.Count
    errors = $errors
}

if ($AsJson) {
    $result | ConvertTo-Json -Depth 14
} else {
    if ($result.valid) {
        Write-Output "VALID: $resolvedReportPath"
    } else {
        Write-Output "INVALID: $resolvedReportPath (errors=$($errors.Count))"
        foreach ($e in $errors) {
            Write-Output "- $($e.path): $($e.message)"
        }
    }
}

if (-not $result.valid) { exit 1 }
exit 0
