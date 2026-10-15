# kimo-gui — Kimo spectator

Read-only live view of engine tool execution. Direction A: Darkroom (mockup A).

**Read-only by construction:** the frontend registers zero Tauri IPC commands;
it can only listen to events. There is no code path from the UI back to the
engine.

## Architecture

```
engine (kimo-mcp / script) --JSONL over TCP 127.0.0.1:7421--> tauri shell (src/main.rs)
                                                             | tauri events
                                                             v
                                                       webview (ui/)
```

The shell binds loopback only and forwards each parsed event as an
`engine-event`. All assets (fonts, three.js) are vendored; the app makes no
network calls beyond the loopback feed.

## Event schema

```json
{"type":"tool_start","id":"c7","name":"color_grade","x":290,"y":40,
 "inputs":[{"port":"in","from":"load_media:out"}],"args":{"gamma":"1.04"}}
{"type":"tool_end","id":"c7","name":"color_grade","dur_ms":14,"status":"ok"}
{"type":"project_changed","rev":214}
{"type":"frame_rendered","frame":142}
{"type":"viewport_tint","color":14497319}
```

`inputs[].from` is `"<tool>:<port>"` and derives the pipeline graph: a tool
appears in the graph when it consumes another tool's output or carries an
explicit `x`/`y` slot; pure calls (search_tools, add_clip) stay in the call
log only.

## Frontend (ui/)

Darkroom tokens (`--ink #0e0e12`, `--surface #15151b`, `--cream #f4f0e8`,
`--orange #ff5c28`, plus mint/lavender/pink/brown/olive accents), three
vendored OFL fonts (Instrument Serif, Bricolage Grotesque, Martian Mono),
three.js 0.180.0 (MIT, vendored with LICENSE).

- **Viewport**: three.js preview on a dirty-flag renderer — the GPU is
  touched only when the scene changes, capped at 30 fps, and the rAF loop
  stops entirely when clean (zero idle GPU).
- **Node graph**: derived from tool inputs; read-only; the live path is
  animated while a tool runs; presenter mode scrolls the active card into
  view.
- **Call log / pass stack / inspector / timeline**: all derive from the same
  event stream; the timeline parks its playhead at the end (zero idle).

## Idle proof (tests/idle_proof.py)

The render loop must cost nothing when nothing happens. One command proves it
in headless Chromium (serves this crate itself on a free loopback port):

    pip install playwright && python3 -m playwright install chromium   # once
    python3 crates/kimo-gui/tests/idle_proof.py          # 10 s per phase, exit 1 on failure

It counts requestAnimationFrame callbacks and timer callbacks per phase and logs
the page's main-thread task time and the renderer process CPU time:

| phase | rAF | timers |
|---|---|---|
| visible, live session, no events (after settle) | 0 | 1/s: the elapsed clock (text only) |
| visible, finished/frozen session | 0 | 0 |
| hidden, engine events still arriving (20/s) | 0 | 0 (events folded into state, not painted) |
| hidden, no events | 0 | 0 |
| visible again | exactly 1 (catch-up paint with the newest state) | - |

Hidden is produced by overriding `document.visibilityState` and dispatching the real
`visibilitychange` event; Chromium keeps running rAF for the page, so any frame the
app requested would still fire and be counted. A running tool call's duration ticks
at 10 Hz (text only) while visible; that is the only other timer.

## Build & run (needs webkit2gtk 4.1)

Ubuntu 22.04: `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev`
then `cargo tauri dev` (or `cargo build` for the shell). In another shell:
`python3 tools/feed_demo.py` to stream the mockup-A scenario.

NOTE: the shell has not been compiled on the dev box this was authored on
(no webkit2gtk-4.1-dev); the frontend is verified in headless Chromium.

## Transports (ui/bridge.js)

One ingestion path, three bindings:

1. **Tauri IPC** — inside the shell; events come from the loopback TCP/JSONL
   listener in `src/main.rs`. The shell never uses SSE.
2. **localhost SSE** — plain-browser development. Run
   `python3 tools/sse_feed.py` (stdlib only, `http://127.0.0.1:7422/events`,
   holds the stream open with keep-alives), serve `ui/` and open
   `http://127.0.0.1:8123/index.html?feed=sse`. The pill reads
   `live · sse loopback :7422`. `?sse=PORT` overrides the port.
3. **simulated** — with neither reachable (or `?feed=sim`), the built-in
   mockup-A replay runs and the pill reads `dev · simulated feed`.

Simulated data is always labeled; every binding announces itself in the
status pill.

## Licenses

Fonts: SIL Open Font License 1.1 (see `fonts/*/OFL.txt`).
three.js: MIT (see `ui/vendor/THREE-LICENSE.md`).
Rust dependencies: **audit pending** — `Cargo.lock` is intentionally not
committed until the dependency-tree license decision lands.
