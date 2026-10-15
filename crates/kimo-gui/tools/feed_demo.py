#!/usr/bin/env python3
"""Feed a running Kimo spectator the mockup-A scenario over loopback JSONL.

Usage: cargo tauri dev   (in another shell, on a box with webkit2gtk-4.1-dev)
       python3 tools/feed_demo.py [--port 7421]

The spectator binds 127.0.0.1:7421; this script connects and streams one
newline-delimited JSON event per line. Stdlib only, no dependencies.
"""
import json, socket, sys, time

PORT = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 7421

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

def main():
    s = socket.create_connection(("127.0.0.1", PORT))
    print(f"connected to spectator on 127.0.0.1:{PORT}, streaming {len(SEQ)} events")
    for typ, body in SEQ:
        s.sendall((json.dumps({"type": typ, **body}) + "\n").encode())
        time.sleep(0.5 if typ == "tool_start" else 0.9)
    s.close()
    print("done")

if __name__ == "__main__":
    main()
