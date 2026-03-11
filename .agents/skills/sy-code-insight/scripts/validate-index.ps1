[CmdletBinding()]
param(
    [string]$IndexPath = ".ai/index.json"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Fail($Message) {
    Write-Error $Message
    exit 1
}

if (-not (Test-Path -LiteralPath $IndexPath)) {
    Fail "Index file not found: $IndexPath"
}

try {
    $json = Get-Content -LiteralPath $IndexPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
} catch {
    Fail "Invalid JSON: $IndexPath"
}

if (-not $json.ContainsKey('schema_name') -or $json.schema_name -ne 'sy.index') {
    Fail "schema_name must be sy.index"
}
if (-not $json.ContainsKey('files') -or $json.files -isnot [hashtable]) {
    Fail "files must be an object keyed by relative path"
}

$errors = New-Object System.Collections.Generic.List[string]
foreach ($key in $json.files.Keys) {
    $entry = $json.files[$key]
    if ($entry -isnot [hashtable]) { $errors.Add("${key}: entry must be object") ; continue }
    if (-not $entry.ContainsKey('path') -or [string]::IsNullOrWhiteSpace([string]$entry.path)) { $errors.Add("${key}: missing path") }
    if (-not $entry.ContainsKey('fingerprint') -or [string]::IsNullOrWhiteSpace([string]$entry.fingerprint)) { $errors.Add("${key}: missing fingerprint") }
    if (-not $entry.ContainsKey('status')) { $errors.Add("${key}: missing status") }
    if (-not $entry.ContainsKey('change_type')) { $errors.Add("${key}: missing change_type") }
    if (-not $entry.ContainsKey('understanding') -or $entry.understanding -isnot [hashtable]) {
        $errors.Add("${key}: missing understanding object")
    } else {
        $u = $entry.understanding
        if (-not $u.ContainsKey('confidence')) { $errors.Add("${key}: understanding.confidence missing") }
        if (-not $u.ContainsKey('blind_spots') -or $u.blind_spots -isnot [System.Collections.IEnumerable]) { $errors.Add("${key}: understanding.blind_spots missing") }
    }
    if ($key -ne [string]$entry.path) { $errors.Add("${key}: key/path mismatch") }
}

if ($errors.Count -gt 0) {
    $errors | ForEach-Object { Write-Host "[index-error] $_" }
    Fail "Index validation failed with $($errors.Count) error(s)."
}

Write-Host "Index valid: $IndexPath ($($json.files.Count) entries)"
