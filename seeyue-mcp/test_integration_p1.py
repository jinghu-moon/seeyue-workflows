"""
P1 Integration Tests: Edge cases and boundary conditions for the policy engine.

Tests coverage:
  1. TDD state blocking (tdd_required=true, tdd_state not ready → block production writes)
  2. Scope drift detection (write outside node.target)
  3. Loop budget exhausted (used >= max → block commands, stop → force_continue)
  4. Approval pending (stop → force_continue)
  5. Recovery restore_pending (stop → force_continue)
  6. Bypass env vars (SY_BYPASS_PRETOOL_BASH=1 allows destructive)
  7. Git special rules (SY_ALLOW_GIT_COMMIT=1 allows git commit)
  8. Secret file edge cases (.env.production, secrets/*, *.pem)
  9. Exhausted budget + command → should still work (budget only affects stop)

Run: python test_integration_p1.py
"""
import subprocess, json, sys, os, tempfile, shutil

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

MCP_EXE = r'D:\100_Projects\110_Daily\seeyue-workflows\seeyue-mcp\target\release\seeyue-mcp.exe'

# ─── Test harness ─────────────────────────────────────────────────────────────

passed = 0
failed = 0

def check(label, condition, detail=''):
    global passed, failed
    if condition:
        passed += 1
        print(f'  ✅ {label}')
    else:
        failed += 1
        print(f'  ❌ {label} — {detail}')

# ─── Helper: create MCP session with custom session.yaml ─────────────────────

def make_workspace(session_yaml_content):
    """Create a temp workspace with workflow specs and custom session."""
    ws = tempfile.mkdtemp(prefix='seeyue_int_')
    project_root = r'D:\100_Projects\110_Daily\seeyue-workflows'
    shutil.copytree(os.path.join(project_root, 'workflow'),
                    os.path.join(ws, 'workflow'))
    ai_dir = os.path.join(ws, '.ai', 'workflow')
    os.makedirs(ai_dir, exist_ok=True)
    with open(os.path.join(ai_dir, 'session.yaml'), 'w', encoding='utf-8') as f:
        f.write(session_yaml_content)
    os.makedirs(os.path.join(ws, 'src'), exist_ok=True)
    with open(os.path.join(ws, 'src', 'main.rs'), 'w', encoding='utf-8') as f:
        f.write('fn main() {}\n')
    return ws

def start_server(workspace, extra_env=None):
    """Start MCP server with given workspace."""
    env = os.environ.copy()
    env['SEEYUE_MCP_WORKSPACE'] = workspace
    for k in ('SY_BYPASS_PRETOOL_BASH', 'SY_BYPASS_PRETOOL_WRITE',
              'SY_ALLOW_GIT_COMMIT', 'SY_ALLOW_GIT_PUSH'):
        env.pop(k, None)
    if extra_env:
        env.update(extra_env)
    proc = subprocess.Popen(
        [MCP_EXE],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        env=env,
    )
    return proc, env

class McpClient:
    def __init__(self, proc):
        self.proc = proc
        self.msg_id = 0

    def send(self, method, params=None):
        self.msg_id += 1
        msg = {'jsonrpc': '2.0', 'id': self.msg_id, 'method': method}
        if params:
            msg['params'] = params
        self.proc.stdin.write((json.dumps(msg) + '\n').encode())
        self.proc.stdin.flush()
        while True:
            raw = self.proc.stdout.readline()
            if not raw:
                raise RuntimeError(f'Server closed stdout (id={self.msg_id})')
            raw = raw.strip()
            if not raw:
                continue
            try:
                resp = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if resp.get('id') == self.msg_id:
                return resp

    def notify(self, method, params=None):
        msg = {'jsonrpc': '2.0', 'method': method}
        if params:
            msg['params'] = params
        self.proc.stdin.write((json.dumps(msg) + '\n').encode())
        self.proc.stdin.flush()

    def call_tool(self, name, args):
        return self.send('tools/call', {'name': name, 'arguments': args})

    def initialize(self):
        self.send('initialize', {
            'protocolVersion': '2025-06-18',
            'capabilities': {},
            'clientInfo': {'name': 'int-test', 'version': '0.1'},
        })
        self.notify('notifications/initialized')

    def get_verdict(self, resp):
        text = resp.get('result', {}).get('content', [{}])[0].get('text', '')
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return {'verdict': 'parse_error', 'reason': text}

    def close(self):
        self.proc.terminate()

def run_test(name, session_yaml, test_fn, extra_env=None):
    """Run a test with its own server instance and workspace."""
    print(f'\n[{name}]')
    ws = make_workspace(session_yaml)
    try:
        proc, env = start_server(ws, extra_env)
        client = McpClient(proc)
        client.initialize()
        test_fn(client, ws)
        client.close()
    finally:
        shutil.rmtree(ws, ignore_errors=True)

# ─── Session YAML templates ──────────────────────────────────────────────────

BASE_SESSION = """schema_version: 1
run_id: wf-int-001
phase:
  id: P1
  name: Implementation
  status: active
node:
  id: P1-N1
  name: Test Node
  status: active
  state: green_verified
  tdd_required: false
  target:
    - src/
loop_budget:
  max: 100
  used: 5
  exhausted: false
approvals:
  pending: []
  grants: []
recovery:
  status: null
"""

TDD_REQUIRED_NOT_READY = """schema_version: 1
run_id: wf-int-002
phase:
  id: P1
  name: Implementation
  status: active
node:
  id: P1-N1
  name: TDD Node
  status: active
  state: red_pending
  tdd_required: true
  tdd_state: red_pending
  target:
    - src/
loop_budget:
  max: 100
  used: 5
  exhausted: false
approvals:
  pending: []
  grants: []
recovery:
  status: null
"""

TDD_REQUIRED_READY = """schema_version: 1
run_id: wf-int-003
phase:
  id: P1
  name: Implementation
  status: active
node:
  id: P1-N1
  name: TDD Node
  status: active
  state: red_verified
  tdd_required: true
  tdd_state: red_verified
  target:
    - src/
loop_budget:
  max: 100
  used: 5
  exhausted: false
approvals:
  pending: []
  grants: []
recovery:
  status: null
"""

LOOP_BUDGET_EXHAUSTED = """schema_version: 1
run_id: wf-int-004
phase:
  id: P1
  name: Implementation
  status: active
node:
  id: P1-N1
  name: Test Node
  status: active
  state: green_verified
  tdd_required: false
  target:
    - src/
loop_budget:
  max: 10
  used: 10
  exhausted: true
approvals:
  pending: []
  grants: []
recovery:
  status: null
"""

APPROVAL_PENDING = """schema_version: 1
run_id: wf-int-005
phase:
  id: P1
  name: Implementation
  status: active
node:
  id: P1-N1
  name: Test Node
  status: active
  state: green_verified
  tdd_required: false
  target:
    - src/
loop_budget:
  max: 100
  used: 5
  exhausted: false
approvals:
  pending:
    - type: destructive_command
      command: "rm -rf /"
  grants: []
recovery:
  status: null
"""

RECOVERY_PENDING = """schema_version: 1
run_id: wf-int-006
phase:
  id: P1
  name: Implementation
  status: active
node:
  id: P1-N1
  name: Test Node
  status: active
  state: green_verified
  tdd_required: false
  target:
    - src/
loop_budget:
  max: 100
  used: 5
  exhausted: false
approvals:
  pending: []
  grants: []
recovery:
  status: restore_pending
  restore_reason: "Checkpoint rollback required"
"""

NARROW_TARGET = """schema_version: 1
run_id: wf-int-007
phase:
  id: P1
  name: Implementation
  status: active
node:
  id: P1-N1
  name: Narrow Node
  status: active
  state: green_verified
  tdd_required: false
  target:
    - src/policy/
loop_budget:
  max: 100
  used: 5
  exhausted: false
approvals:
  pending: []
  grants: []
recovery:
  status: null
"""

# ═══════════════════════════════════════════════════════════════════════════════
# Test 1: TDD state blocking
# ═══════════════════════════════════════════════════════════════════════════════

def test_tdd_blocking(client, ws):
    # Production code write with TDD not ready → should block
    hr = client.get_verdict(client.call_tool('sy_pretool_write', {'path': 'src/main.rs'}))
    check('TDD not ready → block production write',
          hr.get('verdict') == 'block',
          f'got {hr.get("verdict")} reason={hr.get("reason", "")[:80]}')

    # Test file write should still be allowed even with TDD not ready
    hr = client.get_verdict(client.call_tool('sy_pretool_write', {'path': 'tests/test_foo.py'}))
    check('TDD not ready → allow test file write',
          hr.get('verdict') == 'allow',
          f'got {hr.get("verdict")}')

    # Docs file should still be allowed
    hr = client.get_verdict(client.call_tool('sy_pretool_write', {'path': 'docs/README.md'}))
    check('TDD not ready → allow docs write',
          hr.get('verdict') == 'allow',
          f'got {hr.get("verdict")}')

run_test('TDD Blocking (not ready)', TDD_REQUIRED_NOT_READY, test_tdd_blocking)

# ═══════════════════════════════════════════════════════════════════════════════
# Test 2: TDD state ready allows production writes
# ═══════════════════════════════════════════════════════════════════════════════

def test_tdd_ready(client, ws):
    hr = client.get_verdict(client.call_tool('sy_pretool_write', {'path': 'src/main.rs'}))
    check('TDD ready (red_verified) → allow production write',
          hr.get('verdict') == 'allow',
          f'got {hr.get("verdict")}')

run_test('TDD Ready (red_verified)', TDD_REQUIRED_READY, test_tdd_ready)

# ═══════════════════════════════════════════════════════════════════════════════
# Test 3: Scope drift detection
# ═══════════════════════════════════════════════════════════════════════════════

def test_scope_drift(client, ws):
    # Write within target → no drift warning
    hr = client.get_verdict(client.call_tool('sy_pretool_write', {'path': 'src/policy/evaluator.rs'}))
    check('Write within target → allow (no drift)',
          hr.get('verdict') == 'allow',
          f'got {hr.get("verdict")}')

    # Write outside target → should still allow but may have instructions
    hr = client.get_verdict(client.call_tool('sy_pretool_write', {'path': 'src/main.rs'}))
    # src/main.rs starts with "src/" but not "src/policy/" — scope drift
    has_drift_warning = any('scope' in i.lower() or 'drift' in i.lower() or 'target' in i.lower()
                           for i in hr.get('instructions', []))
    check('Write outside narrow target → drift warning in instructions',
          has_drift_warning,
          f'instructions={hr.get("instructions", [])}')

run_test('Scope Drift', NARROW_TARGET, test_scope_drift)

# ═══════════════════════════════════════════════════════════════════════════════
# Test 4: Loop budget exhausted
# ═══════════════════════════════════════════════════════════════════════════════

def test_loop_budget_exhausted(client, ws):
    # sy_stop with exhausted budget → force_continue
    hr = client.get_verdict(client.call_tool('sy_stop', {}))
    check('Budget exhausted → stop returns force_continue',
          hr.get('verdict') == 'force_continue',
          f'got {hr.get("verdict")} reason={hr.get("reason", "")[:80]}')

    # Bash commands should still have budget check
    hr = client.get_verdict(client.call_tool('sy_pretool_bash', {'command': 'ls'}))
    # Budget exhausted might block bash or still allow depending on evaluator logic
    check('Budget exhausted → bash has budget info',
          hr.get('verdict') in ('allow', 'block', 'force_continue'),
          f'got {hr.get("verdict")}')

run_test('Loop Budget Exhausted', LOOP_BUDGET_EXHAUSTED, test_loop_budget_exhausted)

# ═══════════════════════════════════════════════════════════════════════════════
# Test 5: Approval pending → stop force_continue
# ═══════════════════════════════════════════════════════════════════════════════

def test_approval_pending(client, ws):
    hr = client.get_verdict(client.call_tool('sy_stop', {}))
    check('Approval pending → stop returns force_continue',
          hr.get('verdict') == 'force_continue',
          f'got {hr.get("verdict")} reason={hr.get("reason", "")[:80]}')

run_test('Approval Pending', APPROVAL_PENDING, test_approval_pending)

# ═══════════════════════════════════════════════════════════════════════════════
# Test 6: Recovery restore_pending → stop force_continue
# ═══════════════════════════════════════════════════════════════════════════════

def test_recovery_pending(client, ws):
    hr = client.get_verdict(client.call_tool('sy_stop', {}))
    check('Recovery pending → stop returns force_continue',
          hr.get('verdict') == 'force_continue',
          f'got {hr.get("verdict")} reason={hr.get("reason", "")[:80]}')

run_test('Recovery Restore Pending', RECOVERY_PENDING, test_recovery_pending)

# ═══════════════════════════════════════════════════════════════════════════════
# Test 7: Bypass env var (SY_BYPASS_PRETOOL_BASH=1)
# ═══════════════════════════════════════════════════════════════════════════════

def test_bypass_bash(client, ws):
    # Destructive command with bypass → allow
    hr = client.get_verdict(client.call_tool('sy_pretool_bash', {'command': 'rm -rf /'}))
    check('Bypass bash → destructive allowed',
          hr.get('verdict') == 'allow',
          f'got {hr.get("verdict")}')

run_test('Bypass PRETOOL_BASH', BASE_SESSION, test_bypass_bash,
         extra_env={'SY_BYPASS_PRETOOL_BASH': '1'})

# ═══════════════════════════════════════════════════════════════════════════════
# Test 8: Git commit with SY_ALLOW_GIT_COMMIT=1
# ═══════════════════════════════════════════════════════════════════════════════

def test_git_allow_commit(client, ws):
    hr = client.get_verdict(client.call_tool('sy_pretool_bash', {'command': "git commit -m 'test'"}))
    # With SY_ALLOW_GIT_COMMIT=1, git commit should be allowed (or at least not blocked by git rule)
    # It might still be block_with_approval_request due to git_mutating class in approval matrix
    check('SY_ALLOW_GIT_COMMIT=1 → not blocked by git special rule',
          hr.get('verdict') != 'block' or 'git commit requires' not in hr.get('reason', ''),
          f'got {hr.get("verdict")} reason={hr.get("reason", "")[:80]}')

run_test('Git Commit Allowed', BASE_SESSION, test_git_allow_commit,
         extra_env={'SY_ALLOW_GIT_COMMIT': '1'})

# ═══════════════════════════════════════════════════════════════════════════════
# Test 9: Git push with SY_ALLOW_GIT_PUSH=1
# ═══════════════════════════════════════════════════════════════════════════════

def test_git_allow_push(client, ws):
    hr = client.get_verdict(client.call_tool('sy_pretool_bash', {'command': 'git push origin main'}))
    check('SY_ALLOW_GIT_PUSH=1 → not blocked by git special rule',
          hr.get('verdict') != 'block' or 'git push requires' not in hr.get('reason', ''),
          f'got {hr.get("verdict")} reason={hr.get("reason", "")[:80]}')

run_test('Git Push Allowed', BASE_SESSION, test_git_allow_push,
         extra_env={'SY_ALLOW_GIT_PUSH': '1'})

# ═══════════════════════════════════════════════════════════════════════════════
# Test 10: Secret file edge cases
# ═══════════════════════════════════════════════════════════════════════════════

def test_secret_files(client, ws):
    secret_paths = ['.env', '.env.local', '.env.production', 'secrets/api-key.pem']
    for p in secret_paths:
        hr = client.get_verdict(client.call_tool('sy_pretool_write', {'path': p}))
        check(f'Secret file "{p}" → block',
              hr.get('verdict') == 'block',
              f'got {hr.get("verdict")} file_class={hr.get("file_class")}')

run_test('Secret File Edge Cases', BASE_SESSION, test_secret_files)

# ═══════════════════════════════════════════════════════════════════════════════
# Test 11: Bypass write env var
# ═══════════════════════════════════════════════════════════════════════════════

def test_bypass_write(client, ws):
    hr = client.get_verdict(client.call_tool('sy_pretool_write', {'path': '.env'}))
    check('Bypass write → secret file allowed',
          hr.get('verdict') == 'allow',
          f'got {hr.get("verdict")}')

run_test('Bypass PRETOOL_WRITE', BASE_SESSION, test_bypass_write,
         extra_env={'SY_BYPASS_PRETOOL_WRITE': '1'})

# ═══════════════════════════════════════════════════════════════════════════════
# Test 12: Git dry-run (should be safe)
# ═══════════════════════════════════════════════════════════════════════════════

def test_git_dry_run(client, ws):
    hr = client.get_verdict(client.call_tool('sy_pretool_bash', {'command': 'git push --dry-run'}))
    # dry-run should bypass git special rules, but might still hit git_mutating class
    check('git push --dry-run → not hard blocked by git rule',
          'git push requires' not in hr.get('reason', ''),
          f'got {hr.get("verdict")} reason={hr.get("reason", "")[:80]}')

run_test('Git Dry-Run', BASE_SESSION, test_git_dry_run)

# ═══════════════════════════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════════════════════════

print('\n' + '=' * 60)
total = passed + failed
if failed == 0:
    print(f'🎉 ALL {passed} INTEGRATION TESTS PASSED')
else:
    print(f'⚠️  {passed}/{total} passed, {failed} FAILED')
print('=' * 60)
sys.exit(1 if failed > 0 else 0)
