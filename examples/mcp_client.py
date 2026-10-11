"""Tiny MCP stdio client used by examples/tests."""
import json, subprocess
class Kimo:
    def __init__(self, exe="target/release/kimo-mcp"):
        self.p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, stderr=None); self.i = 0
        self.rpc("initialize", {})
    def rpc(self, m, params):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": m, "params": params}) + "\n"); self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())["result"]
    def tools(self): return self.rpc("tools/list", {})["tools"]
    def call(self, tool, **args):
        r = self.rpc("tools/call", {"name": tool, "arguments": args}); t = r["content"][0]["text"]
        if r["isError"]: raise RuntimeError(f"{tool}: {t}")
        return json.loads(t)
