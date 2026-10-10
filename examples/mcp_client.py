"""Tiny MCP stdio client used by examples/tests."""
import json, subprocess
class Kino:
    def __init__(self, exe="target/release/kino-mcp"):
        self.p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True); self.i = 0
        self.rpc("initialize", {})
    def rpc(self, m, params):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": m, "params": params}) + "\n"); self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())["result"]
    def tools(self): return self.rpc("tools/list", {})["tools"]
    def call(self, name, **args):
        r = self.rpc("tools/call", {"name": name, "arguments": args}); t = r["content"][0]["text"]
        if r["isError"]: raise RuntimeError(f"{name}: {t}")
        return json.loads(t)
