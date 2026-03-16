import subprocess, json, sys

proc = subprocess.Popen(
    [r'D:\100_Projects\110_Daily\seeyue-workflows\seeyue-mcp\target\debug\seeyue-mcp.exe'],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL
)

def send(msg):
    line = json.dumps(msg) + '\n'
    proc.stdin.write(line.encode())
    proc.stdin.flush()
    return json.loads(proc.stdout.readline())

# initialize
r = send({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'test','version':'0.1'}}})
print('init:', r['result']['protocolVersion'])

# initialized notification (no response expected)
proc.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n')
proc.stdin.flush()

# tools/list
r = send({'jsonrpc':'2.0','id':2,'method':'tools/list','params':{}})
tools = r['result']['tools']
print(f'tools count: {len(tools)}')
for t in tools:
    print(f'  - {t["name"]}: {t["description"][:60]}')

proc.terminate()
