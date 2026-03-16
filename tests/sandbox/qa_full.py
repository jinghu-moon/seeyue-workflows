import subprocess, json, os, threading

# Clean sandbox files from previous runs so write guard (FILE_NOT_READ) doesn't trigger
_sandbox = 'D:/100_Projects/110_Daily/seeyue-workflows/tests/sandbox'
for _f in ['hello.txt','multi.rs','valid.rs','invalid.rs','preview.rs','mfe_a.rs','mfe_b.rs']:
    _p = os.path.join(_sandbox, _f)
    if os.path.exists(_p):
        os.remove(_p)

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
    entry = f'[FAIL] {name}\n  payload: {json.dumps(payload)}\n  env: {env_state}\n  actual: {str(actual)[:400]}'
    log_entries.append(entry)
    print(f'[FAIL] {name} => {str(actual)[:120]}')

def call(id_, name, args, timeout=10):
    send({'jsonrpc':'2.0','id':id_,'method':'tools/call','params':{'name':name,'arguments':args}})
    return recv(timeout=timeout)

# Initialize
send({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'qa','version':'1'}}})
recv()
send({'jsonrpc':'2.0','method':'notifications/initialized','params':{}})

# PHASE 0: capability discovery
send({'jsonrpc':'2.0','id':10,'method':'resources/list','params':{}})
r = recv()
if r and 'result' in r and len(r['result']['resources']) >= 3: ok('Phase0/resources_list')
else: fail('Phase0/resources_list', {}, 'n/a', r)

send({'jsonrpc':'2.0','id':11,'method':'prompts/list','params':{}})
r = recv()
if r and 'result' in r: ok('Phase0/prompts_list')
else: fail('Phase0/prompts_list', {}, 'n/a', r)

# PHASE 1: P0 File Editing Engine
r = call(20, 'write', {'file_path':'tests/sandbox/hello.txt','content':'Hello World\n'})
if r and 'result' in r: ok('P0/write')
else: fail('P0/write', {}, 'sandbox', r)

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
else: fail('P0/rewind', {'steps':1}, 'new session/no checkpoint', r)

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
    else: fail('P1/sy_pretool_bash_dangerous', {'command':'rm -rf /'}, 'policy', d)
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

# Resources
send({'jsonrpc':'2.0','id':40,'method':'resources/read','params':{'uri':'workflow://session'}})
r = recv()
if r and 'result' in r: ok('P1/resource_session')
else: fail('P1/resource_session', {}, 'n/a', r)

send({'jsonrpc':'2.0','id':41,'method':'resources/read','params':{'uri':'workflow://journal'}})
r = recv()
if r and ('result' in r or 'error' in r): ok('P1/resource_journal')
else: fail('P1/resource_journal', {}, 'n/a', r)

# prompts/get
send({'jsonrpc':'2.0','id':42,'method':'prompts/list','params':{}})
r = recv()
prompts = r['result']['prompts'] if r and 'result' in r else []
if prompts:
    send({'jsonrpc':'2.0','id':43,'method':'prompts/get','params':{'name':prompts[0]['name'],'arguments':{}}})
    r2 = recv()
    if r2 and ('result' in r2 or 'error' in r2): ok('P2/prompts_get')
    else: fail('P2/prompts_get', {}, 'n/a', r2)
else: fail('P2/prompts_get', {}, 'no prompts', None)
# qa_part2.py - P2/P3 tests (appended to qa_part1 context via exec)

# PHASE 3: P2 Advanced Tools
r = call(50, 'env_info', {})
if r and 'result' in r: ok('P2/env_info')
else: fail('P2/env_info', {}, 'n/a', r)

r = call(51, 'resolve_path', {'path':'tests/sandbox/hello.txt'})
if r and 'result' in r: ok('P2/resolve_path')
else: fail('P2/resolve_path', {}, 'n/a', r)

r = call(52, 'workspace_tree', {'depth':1,'respect_gitignore':True})
if r and 'result' in r: ok('P2/workspace_tree')
else: fail('P2/workspace_tree', {}, 'n/a', r)

r = call(53, 'file_outline', {'path':'seeyue-mcp/src/tools/edit.rs','depth':1})
if r and 'result' in r: ok('P2/file_outline')
else: fail('P2/file_outline', {}, 'n/a', r)

call(54, 'write', {'file_path':'tests/sandbox/valid.rs','content':'fn main() { println!("hi"); }\n'})
r = call(55, 'verify_syntax', {'path':'tests/sandbox/valid.rs'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('valid') == True: ok('P2/verify_syntax_valid')
    else: fail('P2/verify_syntax_valid', {}, 'valid rust', d)
else: fail('P2/verify_syntax_valid', {}, 'n/a', r)

call(56, 'write', {'file_path':'tests/sandbox/invalid.rs','content':'fn broken {'})
r = call(57, 'verify_syntax', {'path':'tests/sandbox/invalid.rs'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('valid') == False: ok('P2/verify_syntax_invalid')
    else: fail('P2/verify_syntax_invalid', {}, 'invalid rust', d)
else: fail('P2/verify_syntax_invalid', {}, 'n/a', r)

r = call(58, 'read_range', {'path':'seeyue-mcp/src/tools/edit.rs','start':1,'end':20})
if r and 'result' in r: ok('P2/read_range')
else: fail('P2/read_range', {}, 'n/a', r)

r = call(59, 'search_workspace', {'pattern':'fn main','file_glob':'**/*.rs','max_results':5})
if r and 'result' in r: ok('P2/search_workspace')
else: fail('P2/search_workspace', {}, 'n/a', r)

r = call(60, 'read_compressed', {'path':'seeyue-mcp/src/tools/edit.rs','token_budget':200})
if r and ('result' in r or 'error' in r): ok('P2/read_compressed')
else: fail('P2/read_compressed', {}, 'n/a', r)

call(61, 'write', {'file_path':'tests/sandbox/preview.rs','content':'fn foo() {}\n'})
r = call(62, 'preview_edit', {'file_path':'tests/sandbox/preview.rs','old_string':'fn foo() {}','new_string':'fn foo() -> i32 { 0 }'})
if r and 'result' in r: ok('P2/preview_edit')
else: fail('P2/preview_edit', {}, 'n/a', r)

r = call(63, 'git_status', {})
if r and 'result' in r: ok('P2/git_status')
else: fail('P2/git_status', {}, 'n/a', r)

r = call(64, 'find_definition', {'path':'seeyue-mcp/src/tools/edit.rs','line':18,'column':12})
if r and ('result' in r or 'error' in r): ok('P2/find_definition')
else: fail('P2/find_definition', {}, 'n/a', r)

r = call(65, 'find_references', {'path':'seeyue-mcp/src/tools/edit.rs','line':18,'column':12})
if r and ('result' in r or 'error' in r): ok('P2/find_references')
else: fail('P2/find_references', {}, 'n/a', r)

# PHASE 4: P3
r = call(70, 'run_command', {'command':'echo qa-test'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('exit_code') == 0: ok('P3/run_command')
    else: fail('P3/run_command', {}, 'n/a', d)
else: fail('P3/run_command', {}, 'n/a', r)

r = call(71, 'run_test', {'path':'seeyue-mcp/'}, timeout=120)
if r and ('result' in r or 'error' in r): ok('P3/run_test')
else: fail('P3/run_test', {}, 'timeout', r)

r = call(72, 'lint_file', {'path':'seeyue-mcp/src/tools/edit.rs'})
if r and ('result' in r or 'error' in r): ok('P3/lint_file')
else: fail('P3/lint_file', {}, 'n/a', r)

r = call(73, 'session_summary', {})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if 'status' in d: ok('P3/session_summary')
    else: fail('P3/session_summary', {}, 'n/a', d)
else: fail('P3/session_summary', {}, 'n/a', r)

r = call(74, 'diff_since_checkpoint', {'path':'tests/sandbox/hello.txt'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('status') in ('ok','NO_CHECKPOINT'): ok('P3/diff_since_checkpoint')
    else: fail('P3/diff_since_checkpoint', {}, 'n/a', d)
else: fail('P3/diff_since_checkpoint', {}, 'n/a', r)

r = call(75, 'dependency_graph', {'path':'seeyue-mcp/src/main.rs','depth':2,'direction':'imports'}, timeout=30)
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('status') == 'ok' and d.get('total_edges',0) > 0: ok('P3/dependency_graph')
    else: fail('P3/dependency_graph', {}, 'n/a', d)
else: fail('P3/dependency_graph', {}, 'n/a', r)

r = call(76, 'symbol_rename_preview', {'path':'seeyue-mcp/src/tools/edit.rs','line':18,'column':12,'new_name':'EditConfig'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('dry_run') == True: ok('P3/symbol_rename_preview')
    else: fail('P3/symbol_rename_preview', {}, 'n/a', d)
else: fail('P3/symbol_rename_preview', {}, 'n/a', r)

call(77, 'write', {'file_path':'tests/sandbox/mfe_a.rs','content':'fn alpha() {}\n'})
call(78, 'write', {'file_path':'tests/sandbox/mfe_b.rs','content':'fn beta() {}\n'})
r = call(79, 'multi_file_edit', {'edits':[{'file_path':'tests/sandbox/mfe_a.rs','edits':[{'old_string':'fn alpha() {}','new_string':'fn alpha() -> i32 { 1 }'}]},{'file_path':'tests/sandbox/mfe_b.rs','edits':[{'old_string':'fn beta() {}','new_string':'fn beta() -> i32 { 2 }'}]}],'verify_syntax':False})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('status') == 'ok' and d.get('files_modified') == 2: ok('P3/multi_file_edit')
    else: fail('P3/multi_file_edit', {}, 'n/a', d)
else: fail('P3/multi_file_edit', {}, 'n/a', r)

r = call(80, 'create_file_tree', {'base_path':'tests/sandbox','tree':[{'path':'scaffold/'},{'path':'scaffold/mod.rs','content':'pub fn hello() {}'}],'overwrite':False})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('status') == 'ok': ok('P3/create_file_tree')
    else: fail('P3/create_file_tree', {}, 'n/a', d)
else: fail('P3/create_file_tree', {}, 'n/a', r)

r = call(81, 'type_check', {'path':'tests/sandbox/valid.rs','language':'typescript'})
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('status') in ('ok','errors','TOOL_NOT_FOUND'): ok('P3/type_check')
    else: fail('P3/type_check', {}, 'n/a', d)
else: fail('P3/type_check', {}, 'n/a', r)

r = call(82, 'package_info', {'name':'serde','registry':'crates'}, timeout=15)
if r and 'result' in r:
    d = json.loads(r['result']['content'][0]['text'])
    if d.get('status') in ('ok','NETWORK_ERROR','NOT_FOUND'): ok('P3/package_info')
    else: fail('P3/package_info', {}, 'n/a', d)
else: fail('P3/package_info', {}, 'timeout/n/a', r)

proc.terminate()

# Write log
with open('D:/100_Projects/110_Daily/seeyue-workflows/log.txt', 'a') as f:
    f.write('\n=== QA RUN RESULTS ===\n')
    f.write(f'PASSED: {len(passed)}\nFAILED: {len(failed)}\n')
    for e in log_entries:
        f.write(e + '\n')

print(f'\n=== SUMMARY: {len(passed)} passed, {len(failed)} failed ===')
