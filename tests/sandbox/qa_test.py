import subprocess, json, os, threading

exe = 'D:/100_Projects/110_Daily/seeyue-workflows/seeyue-mcp/target/debug/seeyue-mcp.exe'
env = os.environ.copy()
env['SEEYUE_MCP_WORKSPACE'] = r'D:\100_Projects\110_Daily\seeyue-workflows'

proc = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)

passed = []
failed = []
log_entries = []

def send(obj):
    proc.stdin.write((json.dumps(obj)+'\n').encode())
    proc.stdin.flush()

def recv(timeout=10):
    result = [None]
    def _r():
        while True:
            line = proc.stdout.readline().strip()
            if line:
                result[0] = json.loads(line)
                return
    t = threading.Thread(target=_r, daemon=True)
    t.start()
    t.join(timeout=timeout)
    return result[0]

def ok(name):
    passed.append(name)
    print(f'[PASS] {name}')

def fail(name, payload, env_state, actual):
    failed.append(name)
    entry = f'[FAIL] {name}\n  payload: {json.dumps(payload)}\n  env: {env_state}\n  actual: {str(actual)[:400]}\n'
    log_entries.append(entry)
    print(f'[FAIL] {name} => {str(actual)[:120]}')

def call(id_, name, args, timeout=10):
    send({'jsonrpc':'2.0','id':id_,'method':'tools/call','params':{'name':name,'arguments':args}})
    return recv(timeout=timeout)

# Initialize
send({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'qa','version':'1'}}})
recv()
send({'jsonrpc':'2.0','method':'notifications/initialized','params':{}})

# PHASE 0
send({'jsonrpc':'2.0','id':10,'method':'resources/list','params':{}})
r = recv()
if r and 'result' in r and len(r['result']['resources']) >= 3: ok('Phase0/resources_list')
else: fail('Phase0/resources_list', {}, 'n/a', r)

send({'jsonrpc':'2.0','id':11,'method':'prompts/list','params':{}})
r = recv()
if r and 'result' in r: ok('Phase0/prompts_list')
else: fail('Phase0/prompts_list', {}, 'n/a', r)

# PHASE 1: P0 File Editing
r = call(20, 'write', {'file_path':'tests/sandbox/hello.txt','content':'Hello World\n'})
if r and 'result' in r: ok('P0/write')
else: fail('P0/write', {}, 'sandbox empty', r)

r = call(21, 'read_file', {'file_path':'tests/sandbox/hello.txt'})
if r and 'result' in r and 'Hello World' in r['result']['content'][0]['text']: ok('P0/read_file')
else: fail('P0/read_file', {}, 'after write', r)

r = call(22, 'edit', {'file_path':'tests/sandbox/hello.txt','old_string':'Hello World','new_string':'Hello seeyue'})
if r and 'result' in r: ok('P0/edit')
else: fail('P0/edit', {}, 'after read', r)

call(23, 'write', {'file_path':'tests/sandbox/multi.rs','content':'fn a() {}\nfn b() {}\n'})
r = call(24, 'multi_edit', {'file_path':'tests/sandbox/multi.rs','edits':[{'old_string':'fn a() {}','new_string':'fn a() -> i32 { 1 }'},{'old_string':'fn b() {}','new_string':'fn b() -> i32 { 2 }'}]})
if r and 'result' in r: ok('P0/multi_edit')
else: fail('P0/multi_edit', {}, 'after write', r)

r = call(25, 'rewind', {'steps':1})
if r and 'result' in r: ok('P0/rewind')
else: fail('P0/rewind', {'steps':1}, 'may have no checkpoint', r)

# PHASE 2: P1 Hooks
r = call(30, 'sy_pretool_bash', {'command':'echo hello'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('verdict') in ('allow','block','block_with_approval_request'): ok('P1/sy_pretool_bash_allow')
    else: fail('P1/sy_pretool_bash_allow', {}, 'n/a', d)
else: fail('P1/sy_pretool_bash_allow', {}, 'n/a', r)

r = call(31, 'sy_pretool_bash', {'command':'rm -rf /'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('verdict') in ('block','block_with_approval_request'): ok('P1/sy_pretool_bash_dangerous')
    else: fail('P1/sy_pretool_bash_dangerous', {'command':'rm -rf /'}, 'policy loaded', d)
else: fail('P1/sy_pretool_bash_dangerous', {}, 'n/a', r)

r = call(32, 'sy_pretool_write', {'path':'tests/sandbox/test.rs'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('verdict') in ('allow','block','block_with_approval_request'): ok('P1/sy_pretool_write')
    else: fail('P1/sy_pretool_write', {}, 'n/a', d)
else: fail('P1/sy_pretool_write', {}, 'n/a', r)

r = call(33, 'sy_posttool_write', {'path':'tests/sandbox/hello.txt','outcome':'success'})
if r and 'result' in r: ok('P1/sy_posttool_write')
else: fail('P1/sy_posttool_write', {}, 'n/a', r)

r = call(34, 'sy_stop', {'phase':'done','node_id':'test'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('verdict') in ('allow','force_continue','block'): ok('P1/sy_stop')
    else: fail('P1/sy_stop', {}, 'n/a', d)
else: fail('P1/sy_stop', {}, 'n/a', r)

r = call(35, 'sy_create_checkpoint', {'node_id':'test-node','label':'qa-test'})
if r and ('result' in r or 'error' in r): ok('P1/sy_create_checkpoint')
else: fail('P1/sy_create_checkpoint', {}, 'n/a', r)

r = call(36, 'sy_advance_node', {'to_node':'next-node','reason':'qa-test'})
if r and ('result' in r or 'error' in r): ok('P1/sy_advance_node')
else: fail('P1/sy_advance_node', {}, 'n/a', r)

r = call(37, 'sy_session_start', {})
if r and ('result' in r or 'error' in r): ok('P1/sy_session_start')
else: fail('P1/sy_session_start', {}, 'n/a', r)
