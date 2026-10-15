#!/usr/bin/env python3
"""Zero-dependency localhost SSE feed for plain-browser spectator development.

Serves http://127.0.0.1:7422/events as text/event-stream and replays the
mockup-A scenario to every connection. Stdlib only. The spectator frontend
binds to it with ?feed=sse (or by default when no Tauri shell is present);
the Tauri shell never uses this transport.
"""
import json, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = 7422
# Same scenario as feed_demo.py; kept as a literal list so both transports
# can never drift from the documented schema in README.md.
SEQ = [
    ("tool_start", {"id": "c1", "name": "load_media", "x": 60, "y": 40}),
    ("tool_end",   {"id": "c1", "name": "load_media", "dur_ms": 96, "status": "ok"}),
    ("tool_start", {"id": "c2", "name": "text_layer", "x": 60, "y": 200}),
    ("tool_end",   {"id": "c2", "name": "text_layer", "dur_ms": 41, "status": "ok"}),
    ("tool_start", {"id": "c3", "name": "color_grade", "x": 290, "y": 40,
                    "inputs": [{"port": "in", "from": "load_media:out"}],
                    "args": {"lift": "0.02 0.01 0.00", "gamma": "1.04",
                             "gain": "1.10 1.02 0.96", "space": "ACEScct"}}),
    ("tool_end",   {"id": "c3", "name": "color_grade", "dur_ms": 14, "status": "ok"}),
    ("tool_start", {"id": "c4", "name": "glass_shatter", "x": 290, "y": 190,
                    "inputs": [{"port": "in", "from": "text_layer:out"},
                               {"port": "mask", "from": "load_media:out"}],
                    "args": {"pieces": 512, "seed": 7}}),
    ("tool_end",   {"id": "c4", "name": "glass_shatter", "dur_ms": 212, "status": "ok"}),
    ("tool_start", {"id": "c5", "name": "composite", "x": 540, "y": 110,
                    "inputs": [{"port": "bg", "from": "color_grade:out"},
                               {"port": "fg", "from": "glass_shatter:out"}],
                    "args": {"mode": "over"}}),
    ("tool_start", {"id": "c6", "name": "set_keyframe", "args": {"track": "V3", "t": "00:06"}}),
    ("tool_end",   {"id": "c6", "name": "set_keyframe", "dur_ms": 1, "status": "ok"}),
    ("tool_start", {"id": "c7", "name": "render_frame", "args": {"f": 142}}),
    ("viewport_tint", {"color": 0xDD5227}),
    ("tool_end",   {"id": "c7", "name": "render_frame", "dur_ms": 380, "status": "ok"}),
    ("frame_rendered", {"frame": 142}),
    ("tool_end",   {"id": "c5", "name": "composite", "dur_ms": 31, "status": "ok"}),
    ("tool_start", {"id": "c8", "name": "export_mp4", "x": 760, "y": 150,
                    "inputs": [{"port": "in", "from": "composite:out"}]}),
    ("tool_end",   {"id": "c8", "name": "export_mp4", "dur_ms": 0, "status": "ok"}),
    ("project_changed", {"rev": 214}),
]

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.split("?")[0] != "/events":
            self.send_error(404); return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Access-Control-Allow-Origin", "http://127.0.0.1:8123")
        self.end_headers()
        for typ, body in SEQ:
            try:
                self.wfile.write(b"data: " + json.dumps({"type": typ, **body}).encode() + b"\n\n")
                self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                return
            time.sleep(0.05 if "--fast" in sys.argv else (0.5 if typ == "tool_start" else 0.9))
        # Hold the stream open after the scenario: EventSource auto-reconnects
        # on close, which would replay duplicates into the call log.
        while True:
            try:
                self.wfile.write(b": keep-alive\n\n"); self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                return
            time.sleep(5)

    def log_message(self, *a):  # quiet
        pass

def burst_events(n):
    for i in range(n):
        yield ("tool_start", {"id": f"b{i}", "name": f"burst_tool_{i%7}", "args": {"i": i}})
        yield ("tool_end", {"id": f"b{i}", "name": f"burst_tool_{i%7}", "dur_ms": i % 40, "status": "ok"})

if __name__ == "__main__":
    if "--burst" in sys.argv:
        n = int(sys.argv[sys.argv.index("--burst") + 1]) if "--burst" in sys.argv[1:] and sys.argv[-1].isdigit() else 500
        class BurstHandler(Handler):
            def do_GET(self):
                if self.path.split("?")[0] != "/events":
                    self.send_error(404); return
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Access-Control-Allow-Origin", "http://127.0.0.1:8123")
                self.end_headers()
                for typ, body in burst_events(n):
                    try:
                        self.wfile.write(b"data: " + json.dumps({"type": typ, **body}).encode() + b"\n\n")
                        self.wfile.flush()
                    except (BrokenPipeError, ConnectionResetError):
                        return
                    time.sleep(0.002)
                while True:
                    try:
                        self.wfile.write(b": keep-alive\n\n"); self.wfile.flush()
                    except (BrokenPipeError, ConnectionResetError):
                        return
                    time.sleep(5)
        print(f"sse BURST feed on http://127.0.0.1:{PORT}/events ({n} start+end pairs)")
        ThreadingHTTPServer(("127.0.0.1", PORT), BurstHandler).serve_forever()
    else:
        print(f"sse feed on http://127.0.0.1:{PORT}/events (mockup-A scenario per connection)")
        ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
