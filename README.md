# kino

MCP-first, AI-first video editor. Every operation (cut, transform, keyframe, effect, transition, grade, export) is a registered command; the MCP server's tool list is generated from that registry, so a feature cannot exist without being AI-callable.

- `crates/kino-core`: project model, keyframes, command registry, CPU renderer + effects
- `crates/kino-mcp`: MCP server (stdio JSON-RPC)
- `examples/`: Python MCP client and a graded-sequence demo built purely from tool calls

```
cargo build --release
python3 examples/demo_grade.py
```

Requires `ffmpeg`/`ffprobe` on PATH (decode/encode). Status: early. See ROADMAP.md.
