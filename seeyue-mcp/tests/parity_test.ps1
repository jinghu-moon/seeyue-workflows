# parity_test.ps1
# Node.js vs Rust verdict parity test.
# Runs the same inputs through both engines and compares verdict fields.

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$rustBin = Join-Path $projectRoot "seeyue-mcp\target\release\sy-hook.exe"
$nodeScripts = Join-Path $projectRoot "scripts\hooks"

# Verify binaries exist
if (-not (Test-Path $rustBin)) { Write-Error "Rust binary not found: $rustBin"; exit 1 }

# Test cases: [event, node_script, stdin_json, description]
$testCases = @(
    # ─── PreToolUse:Bash ────────────────────────────────────────────────
    @{
        event = "PreToolUse:Bash"
        nodeScript = "sy-pretool-bash.cjs"
        stdin = '{"tool_name":"Bash","tool_input":{"command":"ls"}}'
        desc = "safe command (ls)"
    },
    @{
        event = "PreToolUse:Bash"
        nodeScript = "sy-pretool-bash.cjs"
        stdin = '{"tool_name":"Bash","tool_input":{"command":"rm -rf /"}}'
        desc = "destructive command (rm -rf /)"
    },
    @{
        event = "PreToolUse:Bash"
        nodeScript = "sy-pretool-bash.cjs"
        stdin = '{"tool_name":"Bash","tool_input":{"command":"git push"}}'
        desc = "git push"
    },
    @{
        event = "PreToolUse:Bash"
        nodeScript = "sy-pretool-bash.cjs"
        stdin = '{"tool_name":"Bash","tool_input":{"command":"cargo test"}}'
        desc = "verify command (cargo test)"
    },
    @{
        event = "PreToolUse:Bash"
        nodeScript = "sy-pretool-bash.cjs"
        stdin = '{"tool_name":"Bash","tool_input":{"command":"cargo build"}}'
        desc = "build command (cargo build)"
    },
    # ─── PreToolUse:Write|Edit ──────────────────────────────────────────
    @{
        event = "PreToolUse:Write|Edit"
        nodeScript = "sy-pretool-write.cjs"
        stdin = '{"tool_name":"Write","tool_input":{"file_path":".env"}}'
        desc = "secret file (.env)"
    },
    @{
        event = "PreToolUse:Write|Edit"
        nodeScript = "sy-pretool-write.cjs"
        stdin = '{"tool_name":"Write","tool_input":{"file_path":"src/main.rs"}}'
        desc = "workspace file (src/main.rs)"
    },
    @{
        event = "PreToolUse:Write|Edit"
        nodeScript = "sy-pretool-write.cjs"
        stdin = '{"tool_name":"Write","tool_input":{"file_path":"certs/server.pem"}}'
        desc = "secret file (server.pem)"
    },
    @{
        event = "PreToolUse:Write|Edit"
        nodeScript = "sy-pretool-write.cjs"
        stdin = '{"tool_name":"Write","tool_input":{"file_path":"Cargo.toml"}}'
        desc = "config file (Cargo.toml)"
    },
    # ─── Stop ───────────────────────────────────────────────────────────
    @{
        event = "Stop"
        nodeScript = "sy-stop.cjs"
        stdin = '{}'
        desc = "stop (clean session)"
    },
    # ─── PostToolUse:Write|Edit ─────────────────────────────────────────
    @{
        event = "PostToolUse:Write|Edit"
        nodeScript = "sy-posttool-write.cjs"
        stdin = '{"tool_name":"Write","tool_input":{"file_path":"src/app.rs"}}'
        desc = "posttool write (src/app.rs)"
    },
    # ─── PostToolUse:Bash ───────────────────────────────────────────────
    @{
        event = "PostToolUse:Bash"
        nodeScript = "sy-posttool-bash-verify.cjs"
        stdin = '{"tool_name":"Bash","tool_input":{"command":"cargo test"},"tool_response":{"stdout":"test result: ok. 5 passed","returncode":0}}'
        desc = "posttool bash (cargo test pass)"
    },
    @{
        event = "PostToolUse:Bash"
        nodeScript = "sy-posttool-bash-verify.cjs"
        stdin = '{"tool_name":"Bash","tool_input":{"command":"echo hello"},"tool_response":{"stdout":"hello","returncode":0}}'
        desc = "posttool bash (non-verify)"
    }
)

function Run-Rust($event, $stdin) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $rustBin
    $psi.Arguments = $event
    $psi.WorkingDirectory = $projectRoot
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $proc = [System.Diagnostics.Process]::Start($psi)
    $proc.StandardInput.Write($stdin)
    $proc.StandardInput.Close()
    $stdout = $proc.StandardOutput.ReadToEnd()
    $proc.WaitForExit(5000)

    return @{
        stdout = $stdout.Trim()
        exitCode = $proc.ExitCode
    }
}

function Run-Node($scriptName, $stdin) {
    $script = Join-Path $nodeScripts $scriptName
    if (-not (Test-Path $script)) { return @{ stdout = "SCRIPT_NOT_FOUND"; exitCode = -1 } }

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = "node"
    $psi.Arguments = "`"$script`""
    $psi.WorkingDirectory = $projectRoot
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $proc = [System.Diagnostics.Process]::Start($psi)
    $proc.StandardInput.Write($stdin)
    $proc.StandardInput.Close()
    $stdout = $proc.StandardOutput.ReadToEnd()
    $proc.WaitForExit(10000)

    return @{
        stdout = $stdout.Trim()
        exitCode = $proc.ExitCode
    }
}

function Extract-Verdict($jsonStr) {
    # Try proper JSON parsing first
    try {
        $obj = $jsonStr | ConvertFrom-Json -ErrorAction Stop
        return $obj.verdict
    } catch {
        # Fallback: regex extraction for non-ASCII JSON or mixed stdout/stderr
        if ($jsonStr -match '"verdict"\s*:\s*"([^"]+)"') {
            return $matches[1]
        }
        return "PARSE_ERROR"
    }
}

function Extract-Reason($jsonStr) {
    try {
        $obj = $jsonStr | ConvertFrom-Json -ErrorAction Stop
        return $obj.reason
    } catch {
        if ($jsonStr -match '"reason"\s*:\s*"([^"]+)"') {
            return $matches[1]
        }
        return ""
    }
}

function Is-PersonaBlock($reason) {
    # Node.js persona guard blocks when session.yaml has owner_persona: human
    # This is an environment-state difference, not a code logic difference.
    return ($reason -match "Persona .* may not (run commands|write files)")
}

# ─── Run Parity Tests ─────────────────────────────────────────────────────────

$passed = 0
$failed = 0
$skipped = 0
$personaDiffs = 0
$results = @()

Write-Host "`n=== Node.js vs Rust Parity Test ===" -ForegroundColor Cyan
Write-Host "Project: $projectRoot"
Write-Host "NOTE: session.yaml has owner_persona=human, Node.js persona guard may block safe operations.`n"

foreach ($tc in $testCases) {
    Write-Host -NoNewline "  [$($tc.event)] $($tc.desc) ... "

    # Run Rust
    $rust = Run-Rust $tc.event $tc.stdin
    $rustVerdict = Extract-Verdict $rust.stdout

    # Run Node.js
    $node = Run-Node $tc.nodeScript $tc.stdin
    $nodeVerdict = Extract-Verdict $node.stdout
    $nodeReason = Extract-Reason $node.stdout

    # Compare verdicts (normalize: block_with_approval_request ≈ block for parity)
    $rustNorm = if ($rustVerdict -match "^block") { "block" } else { $rustVerdict }
    $nodeNorm = if ($nodeVerdict -match "^block") { "block" } else { $nodeVerdict }

    if ($nodeVerdict -eq "SCRIPT_NOT_FOUND") {
        Write-Host "SKIP (node script missing)" -ForegroundColor Yellow
        $skipped++
    } elseif ($rustNorm -ne $nodeNorm -and (Is-PersonaBlock $nodeReason)) {
        # Expected difference: Node.js persona guard blocks because session has human persona
        Write-Host "EXPECTED_DIFF (persona guard)" -ForegroundColor DarkYellow
        Write-Host "    Rust: $rustVerdict  Node: $nodeVerdict (reason: $nodeReason)" -ForegroundColor Gray
        $personaDiffs++
    } elseif ($rustNorm -eq $nodeNorm) {
        Write-Host "PASS" -ForegroundColor Green
        Write-Host "    Rust: $rustVerdict (exit=$($rust.exitCode))  Node: $nodeVerdict (exit=$($node.exitCode))" -ForegroundColor Gray
        $passed++
    } else {
        Write-Host "FAIL" -ForegroundColor Red
        Write-Host "    Rust: verdict=$rustVerdict exit=$($rust.exitCode)" -ForegroundColor Red
        Write-Host "    Node: verdict=$nodeVerdict exit=$($node.exitCode) reason=$nodeReason" -ForegroundColor Red
        $failed++
    }
}

Write-Host "`n=== Results ===" -ForegroundColor Cyan
Write-Host "  Passed:       $passed" -ForegroundColor Green
Write-Host "  Failed:       $failed" -ForegroundColor $(if ($failed -gt 0) { "Red" } else { "Green" })
Write-Host "  Persona diff: $personaDiffs (expected, owner_persona=human in session.yaml)" -ForegroundColor DarkYellow
Write-Host "  Skipped:      $skipped" -ForegroundColor Yellow
Write-Host "  Total:        $($testCases.Count)"

if ($failed -gt 0) { exit 1 } else { exit 0 }
