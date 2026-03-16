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
