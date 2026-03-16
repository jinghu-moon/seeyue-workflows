"""
P1.5 E2E Latency Benchmark: MCP stdio round-trip latency for hook tools + resources.

Measures real-world latency including JSON serialization, stdio I/O, and server processing.
Runs each operation N times and computes min / avg / p50 / p95 / p99 / max.

Target: all p99 < 50ms (stdio overhead dominates; pure Rust decision < 1ms).

Run: python test_perf_p1.py
"""
import subprocess, json, sys, os, tempfile, shutil, time, statistics

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

MCP_EXE = r'D:\100_Projects\110_Daily\seeyue-workflows\seeyue-mcp\target\release\seeyue-mcp.exe'

# ─── Workspace setup ─────────────────────────────────────────────────────────

workspace = tempfile.mkdtemp(prefix='seeyue_perf_')

project_root = r'D:\100_Projects\110_Daily\seeyue-workflows'
shutil.copytree(os.path.join(project_root, 'workflow'),
                os.path.join(workspace, 'workflow'))

ai_workflow_dir = os.path.join(workspace, '.ai', 'workflow')
os.makedirs(ai_workflow_dir, exist_ok=True)

with open(os.path.join(ai_workflow_dir, 'session.yaml'), 'w', encoding='utf-8') as f:
    f.write("""schema_version: 1
run_id: wf-perf-001
phase:
  id: P1
  name: Implementation
  status: active
node:
  id: P1-N1
  name: Perf Node
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

src_dir = os.path.join(workspace, 'src')
os.makedirs(src_dir, exist_ok=True)
with open(os.path.join(src_dir, 'main.rs'), 'w', encoding='utf-8') as f:
    f.write('fn main() {}\n')

# ─── MCP client ──────────────────────────────────────────────────────────────

env = os.environ.copy()
env['SEEYUE_MCP_WORKSPACE'] = workspace
for k in ('SY_BYPASS_PRETOOL_BASH', 'SY_BYPASS_PRETOOL_WRITE',
          'SY_ALLOW_GIT_COMMIT', 'SY_ALLOW_GIT_PUSH'):
    env.pop(k, None)

proc = subprocess.Popen(
    [MCP_EXE],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    env=env,
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
    while True:
        raw = proc.stdout.readline()
        if not raw:
            raise RuntimeError(f'Server closed stdout (id={msg_id})')
        raw = raw.strip()
        if not raw:
            continue
        try:
            resp = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if resp.get('id') == msg_id:
            return resp

def notify(method, params=None):
    msg = {'jsonrpc': '2.0', 'method': method}
    if params:
        msg['params'] = params
    proc.stdin.write((json.dumps(msg) + '\n').encode())
    proc.stdin.flush()

def call_tool(tool_name, arguments):
    return send('tools/call', {'name': tool_name, 'arguments': arguments})

# ─── Benchmark harness ───────────────────────────────────────────────────────

class BenchResult:
    def __init__(self, name, durations_ms):
        self.name = name
        self.n = len(durations_ms)
        s = sorted(durations_ms)
        self.min = s[0]
        self.max = s[-1]
        self.avg = statistics.mean(s)
        self.p50 = s[int(0.50 * len(s))]
        self.p95 = s[min(int(0.95 * len(s)), len(s) - 1)]
        self.p99 = s[min(int(0.99 * len(s)), len(s) - 1)]

    def __str__(self):
        return (
            f'{self.name:<55} {self.n:>4} iters | '
            f'avg {self.avg:>7.2f}ms | '
            f'min {self.min:>7.2f}ms | '
            f'p50 {self.p50:>7.2f}ms | '
            f'p95 {self.p95:>7.2f}ms | '
            f'p99 {self.p99:>7.2f}ms | '
            f'max {self.max:>7.2f}ms'
        )

def bench(name, iterations, fn):
    """Run fn() iterations times, measuring wall-clock ms per call."""
    # Warmup (5 iterations to stabilize stdio pipes)
    for _ in range(5):
        fn()
    durations = []
    for _ in range(iterations):
        t0 = time.perf_counter()
        fn()
        durations.append((time.perf_counter() - t0) * 1000.0)
    return BenchResult(name, durations)

# ─── Initialize MCP session ─────────────────────────────────────────────────

send('initialize', {
    'protocolVersion': '2025-06-18',
    'capabilities': {},
    'clientInfo': {'name': 'perf-bench', 'version': '0.1'},
})
notify('notifications/initialized')

# ─── Benchmarks ──────────────────────────────────────────────────────────────

ITERS = 200
results = []
all_pass = True

sep = '=' * 130

print(f'\n{sep}')
print('  P1.5 E2E Latency Benchmark — MCP stdio round-trip')
print(f'  Iterations per test: {ITERS} (+ 5 warmup)')
print(f'{sep}\n')

# ── 1. sy_pretool_bash ────────────────────────────────────────────────────────

print('  [sy_pretool_bash]')

r = bench('sy_pretool_bash("ls -la")           → allow', ITERS,
          lambda: call_tool('sy_pretool_bash', {'command': 'ls -la'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_bash("rm -rf /")          → block', ITERS,
          lambda: call_tool('sy_pretool_bash', {'command': 'rm -rf /'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_bash("git push origin")   → block', ITERS,
          lambda: call_tool('sy_pretool_bash', {'command': 'git push origin main'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_bash("cargo test")        → allow', ITERS,
          lambda: call_tool('sy_pretool_bash', {'command': 'cargo test'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_bash("curl example.com")  → block_approval', ITERS,
          lambda: call_tool('sy_pretool_bash', {'command': 'curl https://example.com'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_bash("sudo chmod 777")    → block', ITERS,
          lambda: call_tool('sy_pretool_bash', {'command': 'sudo chmod 777 /etc/passwd'}))
results.append(r); print(f'    {r}')

print()

# ── 2. sy_pretool_write ───────────────────────────────────────────────────────

print('  [sy_pretool_write]')

r = bench('sy_pretool_write("src/main.rs")      → allow', ITERS,
          lambda: call_tool('sy_pretool_write', {'path': 'src/main.rs'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_write(".env")             → block', ITERS,
          lambda: call_tool('sy_pretool_write', {'path': '.env'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_write(".env.local")       → block', ITERS,
          lambda: call_tool('sy_pretool_write', {'path': '.env.local'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_write("workflow/policy")  → block_approval', ITERS,
          lambda: call_tool('sy_pretool_write', {'path': 'workflow/policy.spec.yaml'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_write("tests/foo.py")     → allow', ITERS,
          lambda: call_tool('sy_pretool_write', {'path': 'tests/test_foo.py'}))
results.append(r); print(f'    {r}')

r = bench('sy_pretool_write("docs/README.md")   → allow', ITERS,
          lambda: call_tool('sy_pretool_write', {'path': 'docs/README.md'}))
results.append(r); print(f'    {r}')

print()

# ── 3. sy_stop ────────────────────────────────────────────────────────────────

print('  [sy_stop]')

r = bench('sy_stop()                            → allow', ITERS,
          lambda: call_tool('sy_stop', {}))
results.append(r); print(f'    {r}')

print()

# ── 4. sy_posttool_write ──────────────────────────────────────────────────────

print('  [sy_posttool_write]')

r = bench('sy_posttool_write("src/main.rs")     → allow', ITERS,
          lambda: call_tool('sy_posttool_write', {
              'path': 'src/main.rs', 'tool': 'edit', 'lines_changed': 3
          }))
results.append(r); print(f'    {r}')

print()

# ── 5. resources/read ─────────────────────────────────────────────────────────

print('  [resources/read]')

r = bench('resources/read workflow://session', ITERS,
          lambda: send('resources/read', {'uri': 'workflow://session'}))
results.append(r); print(f'    {r}')

r = bench('resources/read workflow://task-graph', ITERS,
          lambda: send('resources/read', {'uri': 'workflow://task-graph'}))
results.append(r); print(f'    {r}')

r = bench('resources/read workflow://journal', ITERS,
          lambda: send('resources/read', {'uri': 'workflow://journal'}))
results.append(r); print(f'    {r}')

# ─── Summary ─────────────────────────────────────────────────────────────────

print(f'\n{sep}')
print('  SUMMARY')
print(f'{sep}\n')

# MCP stdio adds significant overhead (process IO, JSON ser/de, pipe buffering).
# Target: p99 < 50ms for all operations (pure Rust < 1ms, stdio adds 1-20ms typical).
THRESHOLD_MS = 50.0

worst_p99 = 0.0
for r in results:
    if r.p99 > worst_p99:
        worst_p99 = r.p99
    if r.p99 >= THRESHOLD_MS:
        all_pass = False
        print(f'  ❌ FAIL  p99={r.p99:.2f}ms > {THRESHOLD_MS}ms  {r.name}')

avg_of_avgs = statistics.mean(r.avg for r in results)
avg_of_p99s = statistics.mean(r.p99 for r in results)

print(f'\n  Total operations benchmarked : {len(results)}')
print(f'  Mean of avg latencies       : {avg_of_avgs:.2f}ms')
print(f'  Mean of p99 latencies       : {avg_of_p99s:.2f}ms')
print(f'  Worst p99                   : {worst_p99:.2f}ms')
print(f'  Target                      : all p99 < {THRESHOLD_MS}ms')
print(f'  Result                      : {"PASS ✅" if all_pass else "FAIL ❌"}')
print(f'\n{sep}\n')

# ─── Cleanup ─────────────────────────────────────────────────────────────────

proc.terminate()
shutil.rmtree(workspace, ignore_errors=True)

sys.exit(0 if all_pass else 1)
