# Release binaries

Assets per tag `vX.Y.Z` (or manual run of the `release` workflow):

- `kimo-<tag>-linux-x86_64-static.tar.gz` - primary. Static musl, runs on any x86_64 Linux.
- `kimo-<tag>-linux-x86_64-glibc.tar.gz` - dynamic glibc (built on Ubuntu 22.04, needs glibc >= 2.35). Optional.
- `.sha256` next to each.

Layout: `bin/kimo-mcp`, `fonts/*.ttf` (+ `fetch.sh`), docs. Keep `fonts/` next to `bin/`;
Kimo finds it from the executable path (also `KIMO_FONTS=<dir>`, `./fonts`, `~/kimo/fonts`, `/usr/share/fonts`).

## What a fresh box needs

- Nothing to link: the default build has no OCIO, no C/C++ runtime deps. `kimo-mcp` speaks MCP over stdio.
- **`ffmpeg` and `ffprobe` on PATH** (required at runtime): all media decode/encode, audio, mattes, export
  shell out to them. They are NOT bundled. Install with e.g. `apt install ffmpeg`.
- Optional: `whisper` on PATH, only for the transcription command.
- Fonts: bundled OFL Google Fonts (downloaded at release time from github.com/google/fonts). If a font is
  missing, Kimo falls back to Montserrat, then `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`.
  A text command with its default font path `/usr/share/fonts/truetype/noto/...` needs that file or an explicit font name.
- Optional env: `KIMO_FONTS`, `KIMO_THREADS` (max 16), guard limits in `guard.rs`.

## OCIO (native-ocio feature) is NOT in release binaries

`--features native-ocio` links the system shared `libOpenColorIO` (OCIO 2.x) and needs `OCIO_INCLUDE_DIR`
and `OCIO_LIB_DIR` at build time (see `docs/OCIO_NATIVE.md`); it cannot be static-musl. Release builds omit it, so
explicit OCIO effects fail closed. No `.ocio` config or data file is shipped or required by default builds;
OCIO configs are user-supplied, self-contained files passed by path. Shipping a native-ocio build would need
a bundled libOpenColorIO plus matching headers on the build runner, which is not set up.

## Cutting a release

    git tag v0.1.0 && git push origin v0.1.0     # builds and publishes
    # or: Actions > release > Run workflow (artifacts only; tick "publish" on a tag ref to release)

Build locally: `cargo build --release --locked -p kimo-mcp --target x86_64-unknown-linux-musl`
(`rustup target add x86_64-unknown-linux-musl` first).
