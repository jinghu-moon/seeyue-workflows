"""
P1 E2E Test: Policy Engine + Hook Tools + Workflow Resources
Tests the complete P1 feature set through MCP protocol.

Test matrix:
  1. initialize → capabilities include resources
  2. tools/list → 11 tools (5 P0 + 6 P1)
  3. resources/list → 3 resources
  4. resources/read workflow://session → returns YAML content
  5. sy_pretool_bash {"command":"ls"} → allow (safe)
  6. sy_pretool_bash {"command":"rm -rf /"} → block (destructive)
  7. sy_pretool_bash {"command":"git push"} → block (git_mutating)
  8. sy_pretool_write {"path":".env"} → block (secret_material)
  9. sy_pretool_write {"path":"src/main.rs"} → allow (workspace_file)
 10. sy_stop {} → allow or force_continue
 11. sy_posttool_write → allow (journal event recorded)
 12. sy_advance_node → allow (session updated)
"""
import subprocess, json, sys, os, tempfile, shutil

# Force UTF-8 output to avoid GBK encoding issues on Windows
sys.stdout.reconfigure(encoding='utf-8', errors='replace')

MCP_EXE = r'D:\100_Projects\110_Daily\seeyue-workflows\seeyue-mcp\target\release\seeyue-mcp.exe'

# Create a temp workspace that mirrors the project structure
workspace = tempfile.mkdtemp(prefix='seeyue_mcp_p1_test_')
print(f'Workspace: {workspace}')

# Copy workflow YAML specs into the temp workspace
project_root = r'D:\100_Projects\110_Daily\seeyue-workflows'
workflow_src = os.path.join(project_root, 'workflow')
workflow_dst = os.path.join(workspace, 'workflow')
shutil.copytree(workflow_src, workflow_dst)

# Create .ai/workflow/ directory for session state
ai_workflow_dir = os.path.join(workspace, '.ai', 'workflow')
os.makedirs(ai_workflow_dir, exist_ok=True)

# Create a minimal session.yaml
session_yaml = os.path.join(ai_workflow_dir, 'session.yaml')
with open(session_yaml, 'w', encoding='utf-8') as f:
    f.write("""schema_version: 1
run_id: wf-test-001
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
loop_budget:
  max: 100
  used: 5
  exhausted: false
approvals:
  pending: []
  grants: []
recovery:
  status: null
""")

# Create a test source file
src_dir = os.path.join(workspace, 'src')
os.makedirs(src_dir, exist_ok=True)
with open(os.path.join(src_dir, 'main.rs'), 'w', encoding='utf-8') as f:
    f.write('fn main() { println!("hello"); }\n')

print('---')

env = os.environ.copy()
env['SEEYUE_MCP_WORKSPACE'] = workspace
# Ensure no bypass env vars are set
env.pop('SY_BYPASS_PRETOOL_BASH', None)
env.pop('SY_BYPASS_PRETOOL_WRITE', None)
env.pop('SY_ALLOW_GIT_COMMIT', None)
env.pop('SY_ALLOW_GIT_PUSH', None)

proc = subprocess.Popen(
    [MCP_EXE],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    env=env
)

msg_id = 0
passed = 0
failed = 0

def send(method, params=None):
    global msg_id
    msg_id += 1
    msg = {'jsonrpc': '2.0', 'id': msg_id, 'method': method}
    if params:
        msg['params'] = params
    line = json.dumps(msg) + '\n'
    proc.stdin.write(line.encode())
    proc.stdin.flush()
    while True:
        raw = proc.stdout.readline()
        if not raw:
            raise RuntimeError(f'Server closed stdout while waiting for id={msg_id}')
        raw = raw.strip()
        if not raw:
            continue
        try:
            resp = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if 'id' not in resp:
            continue
        if resp['id'] == msg_id:
            return resp

def notify(method, params=None):
    msg = {'jsonrpc': '2.0', 'method': method}
    if params:
        msg['params'] = params
    line = json.dumps(msg) + '\n'
    proc.stdin.write(line.encode())
    proc.stdin.flush()

def call_tool(tool_name, arguments):
    return send('tools/call', {'name': tool_name, 'arguments': arguments})

def check(label, condition, detail=''):
    global passed, failed
    if condition:
        passed += 1
        print(f'  ✅ {label}')
    else:
        failed += 1
        print(f'  ❌ {label} — {detail}')

def get_tool_text(resp):
    """Extract text from tool call response."""
    content = resp.get('result', {}).get('content', [])
    if content:
        return content[0].get('text', '')
    return ''

def parse_hook_result(resp):
    """Parse HookResult JSON from tool response."""
    text = get_tool_text(resp)
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return {'verdict': 'parse_error', 'reason': text}

# ═══════════════════════════════════════════════════════════════════════
# Test 1: Initialize — capabilities include resources
# ═══════════════════════════════════════════════════════════════════════
print('\n[1] Initialize')
r = send('initialize', {
    'protocolVersion': '2025-06-18',
    'capabilities': {},
    'clientInfo': {'name': 'p1-e2e-test', 'version': '0.1'}
})
notify('notifications/initialized')

caps = r.get('result', {}).get('capabilities', {})
check('protocolVersion present', 'protocolVersion' in r.get('result', {}))
check('tools capability', 'tools' in caps)
check('resources capability', 'resources' in caps,
      f'capabilities={json.dumps(caps)}')

# ═══════════════════════════════════════════════════════════════════════
# Test 2: tools/list — 11 tools (5 P0 + 6 P1)
# ═══════════════════════════════════════════════════════════════════════
print('\n[2] tools/list')
r = send('tools/list')
tools = r.get('result', {}).get('tools', [])
tool_names = sorted([t['name'] for t in tools])
print(f'  Found {len(tools)} tools: {tool_names}')

expected_p0 = {'read_file', 'write', 'edit', 'multi_edit', 'rewind'}
expected_p1 = {'sy_pretool_bash', 'sy_pretool_write', 'sy_posttool_write',
               'sy_stop', 'sy_create_checkpoint', 'sy_advance_node'}
expected_all = expected_p0 | expected_p1

check(f'tool count >= 11', len(tools) >= 11, f'got {len(tools)}')
for t in expected_all:
    check(f'tool "{t}" exists', t in tool_names, f'missing from {tool_names}')

# ═══════════════════════════════════════════════════════════════════════
# Test 3: resources/list — 3 resources
# ═══════════════════════════════════════════════════════════════════════
print('\n[3] resources/list')
r = send('resources/list')
resources = r.get('result', {}).get('resources', [])
resource_uris = [res['uri'] for res in resources]
print(f'  Found {len(resources)} resources: {resource_uris}')

check('resource count == 3', len(resources) == 3, f'got {len(resources)}')
check('workflow://session', 'workflow://session' in resource_uris)
check('workflow://task-graph', 'workflow://task-graph' in resource_uris)
check('workflow://journal', 'workflow://journal' in resource_uris)

# ═══════════════════════════════════════════════════════════════════════
# Test 4: resources/read workflow://session — returns YAML content
# ═══════════════════════════════════════════════════════════════════════
print('\n[4] resources/read workflow://session')
r = send('resources/read', {'uri': 'workflow://session'})
contents = r.get('result', {}).get('contents', [])
if contents:
    session_content = contents[0].get('text', '')
    print(f'  Content length: {len(session_content)} chars')
    check('session contains run_id', 'wf-test-001' in session_content,
          f'content: {session_content[:100]}')
    check('session contains phase', 'P1' in session_content)
else:
    check('session content returned', False, 'empty contents')

# ═══════════════════════════════════════════════════════════════════════
# Test 5: sy_pretool_bash — safe command → allow
# ═══════════════════════════════════════════════════════════════════════
print('\n[5] sy_pretool_bash: safe command')
r = call_tool('sy_pretool_bash', {'command': 'ls -la'})
hr = parse_hook_result(r)
print(f'  verdict={hr.get("verdict")} reason={hr.get("reason", "")[:80]}')
check('ls → allow', hr.get('verdict') == 'allow')

# ═══════════════════════════════════════════════════════════════════════
# Test 6: sy_pretool_bash — destructive command → block
# ═══════════════════════════════════════════════════════════════════════
print('\n[6] sy_pretool_bash: destructive command')
r = call_tool('sy_pretool_bash', {'command': 'rm -rf /'})
hr = parse_hook_result(r)
print(f'  verdict={hr.get("verdict")} command_class={hr.get("command_class")}')
check('rm -rf → block or block_with_approval_request',
      hr.get('verdict') in ('block', 'block_with_approval_request'),
      f'got {hr.get("verdict")}')
check('command_class=destructive', hr.get('command_class') == 'destructive',
      f'got {hr.get("command_class")}')

# ═══════════════════════════════════════════════════════════════════════
# Test 7: sy_pretool_bash — git push → block (no env var)
# ═══════════════════════════════════════════════════════════════════════
print('\n[7] sy_pretool_bash: git push')
r = call_tool('sy_pretool_bash', {'command': 'git push origin main'})
hr = parse_hook_result(r)
print(f'  verdict={hr.get("verdict")} reason={hr.get("reason", "")[:80]}')
check('git push → block', hr.get('verdict') == 'block',
      f'got {hr.get("verdict")}')

# ═══════════════════════════════════════════════════════════════════════
# Test 8: sy_pretool_write — .env → block (secret_material)
# ═══════════════════════════════════════════════════════════════════════
print('\n[8] sy_pretool_write: secret material')
r = call_tool('sy_pretool_write', {'path': '.env'})
hr = parse_hook_result(r)
print(f'  verdict={hr.get("verdict")} file_class={hr.get("file_class")}')
check('.env → block', hr.get('verdict') == 'block',
      f'got {hr.get("verdict")}')
check('file_class=secret_material', hr.get('file_class') == 'secret_material',
      f'got {hr.get("file_class")}')

# ═══════════════════════════════════════════════════════════════════════
# Test 9: sy_pretool_write — src/main.rs → allow (workspace_file)
# ═══════════════════════════════════════════════════════════════════════
print('\n[9] sy_pretool_write: workspace file')
r = call_tool('sy_pretool_write', {'path': 'src/main.rs'})
hr = parse_hook_result(r)
print(f'  verdict={hr.get("verdict")} file_class={hr.get("file_class")}')
check('src/main.rs → allow', hr.get('verdict') == 'allow',
      f'got {hr.get("verdict")}')

# ═══════════════════════════════════════════════════════════════════════
# Test 10: sy_stop — allow (no blockers in session)
# ═══════════════════════════════════════════════════════════════════════
print('\n[10] sy_stop')
r = call_tool('sy_stop', {})
hr = parse_hook_result(r)
print(f'  verdict={hr.get("verdict")} reason={hr.get("reason", "")[:80]}')
check('stop → allow', hr.get('verdict') == 'allow',
      f'got {hr.get("verdict")}')

# ═══════════════════════════════════════════════════════════════════════
# Test 11: sy_posttool_write — always allow, records journal event
# ═══════════════════════════════════════════════════════════════════════
print('\n[11] sy_posttool_write')
r = call_tool('sy_posttool_write', {
    'path': 'src/main.rs',
    'tool': 'edit',
    'lines_changed': 5,
})
hr = parse_hook_result(r)
print(f'  verdict={hr.get("verdict")} reason={hr.get("reason", "")[:80]}')
check('posttool_write → allow', hr.get('verdict') == 'allow')

# Verify journal was written
journal_path = os.path.join(ai_workflow_dir, 'journal.jsonl')
check('journal.jsonl created', os.path.exists(journal_path))
if os.path.exists(journal_path):
    with open(journal_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    check('journal has entries', len(lines) > 0, f'got {len(lines)} lines')
    if lines:
        last_event = json.loads(lines[-1])
        check('journal event=write_recorded',
              last_event.get('event') == 'write_recorded',
              f'got {last_event.get("event")}')

# ═══════════════════════════════════════════════════════════════════════
# Test 12: sy_advance_node — update session + journal events
# ═══════════════════════════════════════════════════════════════════════
print('\n[12] sy_advance_node')
r = call_tool('sy_advance_node', {
    'node_id': 'P1-N2',
    'name': 'Integration Test Node',
    'status': 'active',
    'tdd_required': False,
    'target': ['src/'],
})
hr = parse_hook_result(r)
print(f'  verdict={hr.get("verdict")} reason={hr.get("reason", "")[:80]}')
check('advance_node → allow', hr.get('verdict') == 'allow')

# Verify session.yaml was updated
with open(session_yaml, 'r', encoding='utf-8') as f:
    updated_session = f.read()
check('session updated with P1-N2', 'P1-N2' in updated_session,
      f'session content: {updated_session[:100]}')

# Verify journal has node events
with open(journal_path, 'r', encoding='utf-8') as f:
    lines = f.readlines()
events = [json.loads(l) for l in lines]
event_types = [e.get('event') for e in events]
check('node_exited in journal', 'node_exited' in event_types,
      f'events: {event_types}')
check('node_entered in journal', 'node_entered' in event_types,
      f'events: {event_types}')

# ═══════════════════════════════════════════════════════════════════════
# Test 13: Additional classification checks
# ═══════════════════════════════════════════════════════════════════════
print('\n[13] Additional command classifications')

# cargo test → verify (safe, no approval needed)
r = call_tool('sy_pretool_bash', {'command': 'cargo test'})
hr = parse_hook_result(r)
check('cargo test → allow', hr.get('verdict') == 'allow')
check('cargo test → command_class=verify',
      hr.get('command_class') == 'verify',
      f'got {hr.get("command_class")}')

# curl → network_sensitive → block_with_approval_request
r = call_tool('sy_pretool_bash', {'command': 'curl https://example.com'})
hr = parse_hook_result(r)
check('curl → block_with_approval_request',
      hr.get('verdict') == 'block_with_approval_request',
      f'got {hr.get("verdict")}')
check('curl → command_class=network_sensitive',
      hr.get('command_class') == 'network_sensitive',
      f'got {hr.get("command_class")}')

# sudo → privileged → block_with_approval_request
r = call_tool('sy_pretool_bash', {'command': 'sudo rm -rf /'})
hr = parse_hook_result(r)
# sudo matches privileged first (higher priority than destructive in rm)
# Actually destructive has higher priority than privileged
check('sudo rm -rf → block/block_with_approval',
      hr.get('verdict') in ('block', 'block_with_approval_request'))

print('\n[14] Additional file classifications')

# .env.local → secret_material → block
r = call_tool('sy_pretool_write', {'path': '.env.local'})
hr = parse_hook_result(r)
check('.env.local → block', hr.get('verdict') == 'block')

# workflow/policy.spec.yaml → security_boundary or critical_policy_file
r = call_tool('sy_pretool_write', {'path': 'workflow/policy.spec.yaml'})
hr = parse_hook_result(r)
check('workflow yaml → block_with_approval_request',
      hr.get('verdict') == 'block_with_approval_request',
      f'got {hr.get("verdict")} file_class={hr.get("file_class")}')

# tests/test_foo.py → test_file → allow
r = call_tool('sy_pretool_write', {'path': 'tests/test_foo.py'})
hr = parse_hook_result(r)
check('test file → allow', hr.get('verdict') == 'allow',
      f'got {hr.get("verdict")}')

# docs/README.md → docs_file → allow
r = call_tool('sy_pretool_write', {'path': 'docs/README.md'})
hr = parse_hook_result(r)
check('docs file → allow', hr.get('verdict') == 'allow',
      f'got {hr.get("verdict")}')

# ═══════════════════════════════════════════════════════════════════════
# Test 15: resources/read workflow://journal — shows events
# ═══════════════════════════════════════════════════════════════════════
print('\n[15] resources/read workflow://journal')
r = send('resources/read', {'uri': 'workflow://journal'})
contents = r.get('result', {}).get('contents', [])
if contents:
    journal_content = contents[0].get('text', '')
    check('journal resource has content', len(journal_content) > 0)
    check('journal contains write_recorded', 'write_recorded' in journal_content)
    check('journal contains node_entered', 'node_entered' in journal_content)
else:
    check('journal content returned', False, 'empty contents')

# ═══════════════════════════════════════════════════════════════════════
# Cleanup
# ═══════════════════════════════════════════════════════════════════════
proc.terminate()
shutil.rmtree(workspace, ignore_errors=True)

# ═══════════════════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════════════════
print('\n' + '=' * 60)
total = passed + failed
if failed == 0:
    print(f'🎉 ALL {passed} P1 E2E TESTS PASSED')
else:
    print(f'⚠️  {passed}/{total} passed, {failed} FAILED')
print('=' * 60)
sys.exit(1 if failed > 0 else 0)
