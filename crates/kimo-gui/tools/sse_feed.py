#!/usr/bin/env python3
"""Zero-dependency localhost SSE feed for plain-browser spectator development.

Serves http://127.0.0.1:7422/events (text/event-stream) and replays the 16
demo scenarios (demo/scenarios.json, one per mockup) to every connection.
Open http://127.0.0.1:8123/ui/index.html?feed=sse with the crate root served
on :8123 (python3 -m http.server 8123 in crates/kimo-gui).

    python3 tools/sse_feed.py [--scenario m05] [--hold 4] [--port 7422]

The Tauri shell never uses this transport.
"""
import argparse, json, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import scenarios

ap = argparse.ArgumentParser()
ap.add_argument("--port", type=int, default=7422)
ap.add_argument("--scenario")
ap.add_argument("--hold", type=float, default=4.0)
A = ap.parse_args()

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        if self.path.split("?")[0] != "/events":
            self.send_response(404); self.end_headers(); return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        try:
            while True:
                for s in scenarios.load():
                    if A.scenario and s["id"] != A.scenario: continue
                    for ev, wait in scenarios.events(s):
                        if wait: time.sleep(wait)
                        self.wfile.write(f"data: {json.dumps(ev)}\n\n".encode()); self.wfile.flush()
                    t = time.time()
                    while time.time() - t < A.hold:
                        time.sleep(1); self.wfile.write(b": keep-alive\n\n"); self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass

print(f"kimo spectator SSE feed on http://127.0.0.1:{A.port}/events")
ThreadingHTTPServer(("127.0.0.1", A.port), H).serve_forever()
