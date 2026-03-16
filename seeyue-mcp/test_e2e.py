"""
P0 E2E Test: read_file -> edit -> read_file (verify) -> rewind -> read_file (verify restore)
Tests the complete file editing lifecycle through MCP protocol.
"""
import subprocess, json, sys, os, tempfile, shutil

# Force UTF-8 output to avoid GBK encoding issues on Windows
sys.stdout.reconfigure(encoding='utf-8', errors='replace')

MCP_EXE = r'D:\100_Projects\110_Daily\seeyue-workflows\seeyue-mcp\target\debug\seeyue-mcp.exe'

# Create a temp workspace with a test file
workspace = tempfile.mkdtemp(prefix='seeyue_mcp_test_')
test_file = os.path.join(workspace, 'test.txt')
with open(test_file, 'w', encoding='utf-8') as f:
    f.write('Hello World\nLine 2\nLine 3\n')

print(f'Workspace: {workspace}')
print(f'Test file: {test_file}')
print('---')

env = os.environ.copy()
env['SEEYUE_MCP_WORKSPACE'] = workspace

proc = subprocess.Popen(
    [MCP_EXE],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    env=env
)

msg_id = 0

def send(method, params=None):
    global msg_id
    msg_id += 1
    msg = {'jsonrpc': '2.0', 'id': msg_id, 'method': method}
    if params:
        msg['params'] = params
    line = json.dumps(msg) + '\n'
    proc.stdin.write(line.encode())
    proc.stdin.flush()
    # Read lines until we get the response with matching id
    # (server may send async notifications in between)
    while True:
        raw = proc.stdout.readline()
        if not raw:
            raise RuntimeError(f'Server closed stdout while waiting for id={msg_id}')
        raw = raw.strip()
        if not raw:
            continue
        # Skip non-JSON lines (ANSI escapes, log output, etc.)
        try:
            resp = json.loads(raw)
        except json.JSONDecodeError:
            print(f'    [non-json] {raw[:120]}')
            continue
        # Skip notifications (no 'id' field)
        if 'id' not in resp:
            print(f'    [notification] {resp.get("method", "unknown")}')
            continue
        if resp['id'] == msg_id:
            return resp
        # Skip responses with different id (shouldn't happen, but be safe)
        print(f'    [unexpected id={resp["id"]}] skipping')

def notify(method, params=None):
    msg = {'jsonrpc': '2.0', 'method': method}
    if params:
        msg['params'] = params
    line = json.dumps(msg) + '\n'
    proc.stdin.write(line.encode())
    proc.stdin.flush()

def call_tool(tool_name, arguments):
    return send('tools/call', {'name': tool_name, 'arguments': arguments})

# ── Step 1: Initialize ──
r = send('initialize', {
    'protocolVersion': '2025-06-18',
    'capabilities': {},
    'clientInfo': {'name': 'e2e-test', 'version': '0.1'}
})
print(f'[1] Initialize: protocolVersion={r["result"]["protocolVersion"]}')
notify('notifications/initialized')

# ── Step 2: read_file ──
r = call_tool('read_file', {'file_path': 'test.txt'})
content_items = r['result']['content']
text = content_items[0]['text'] if content_items else ''
print(f'[2] read_file: {repr(text[:80])}...')
assert 'Hello World' in text, f'Expected "Hello World" in read content'
print('    ✅ read_file OK')

# ── Step 3: edit (replace "Hello World" with "Hello MCP") ──
r = call_tool('edit', {
    'file_path': 'test.txt',
    'old_string': 'Hello World',
    'new_string': 'Hello MCP'
})
edit_result = r['result']['content'][0]['text']
print(f'[3] edit: {edit_result[:100]}')
is_error = r['result'].get('isError', False)
assert not is_error, f'Edit returned error: {edit_result}'
print('    ✅ edit OK')

# ── Step 4: read_file (verify edit applied) ──
r = call_tool('read_file', {'file_path': 'test.txt'})
text = r['result']['content'][0]['text']
print(f'[4] read_file after edit: {repr(text[:80])}...')
assert 'Hello MCP' in text, f'Expected "Hello MCP" after edit'
assert 'Hello World' not in text, f'Old string should be gone'
print('    ✅ edit verified')

# ── Step 5: rewind ──
r = call_tool('rewind', {'steps': 1})
rewind_result = r['result']['content'][0]['text']
print(f'[5] rewind: {rewind_result[:100]}')
is_error = r['result'].get('isError', False)
assert not is_error, f'Rewind returned error: {rewind_result}'
print('    ✅ rewind OK')

# ── Step 6: read_file (verify rewind restored original) ──
r = call_tool('read_file', {'file_path': 'test.txt'})
text = r['result']['content'][0]['text']
print(f'[6] read_file after rewind: {repr(text[:80])}...')
assert 'Hello World' in text, f'Expected "Hello World" restored after rewind'
assert 'Hello MCP' not in text, f'"Hello MCP" should be reverted'
print('    ✅ rewind verified - original content restored')

# ── Step 7: multi_edit test ──
# First do a fresh read to update cache
r = call_tool('read_file', {'file_path': 'test.txt'})

r = call_tool('multi_edit', {
    'file_path': 'test.txt',
    'edits': [
        {'old_string': 'Hello World', 'new_string': 'Bonjour MCP'},
        {'old_string': 'Line 2', 'new_string': 'Ligne 2'},
        {'old_string': 'Line 3', 'new_string': 'Ligne 3'}
    ]
})
multi_result = r['result']['content'][0]['text']
is_error = r['result'].get('isError', False)
print(f'[7] multi_edit: {multi_result[:120]}')
assert not is_error, f'multi_edit returned error: {multi_result}'
print('    ✅ multi_edit OK')

# ── Step 8: verify multi_edit ──
r = call_tool('read_file', {'file_path': 'test.txt'})
text = r['result']['content'][0]['text']
print(f'[8] read_file after multi_edit: {repr(text[:80])}...')
assert 'Bonjour MCP' in text
assert 'Ligne 2' in text
assert 'Ligne 3' in text
print('    ✅ multi_edit verified')

# ── Step 9: rewind multi_edit ──
r = call_tool('rewind', {'steps': 1})
r = call_tool('read_file', {'file_path': 'test.txt'})
text = r['result']['content'][0]['text']
assert 'Hello World' in text
assert 'Line 2' in text
print(f'[9] rewind multi_edit: restored to original')
print('    ✅ rewind multi_edit verified')

# ── Cleanup ──
proc.terminate()
shutil.rmtree(workspace, ignore_errors=True)

print('\n' + '=' * 50)
print('🎉 ALL P0 E2E TESTS PASSED')
print('=' * 50)
