#!/usr/bin/env python3
"""1,000-clip timeline: frame times, JS heap and DOM size while a big live timeline keeps changing.

Builds a live edit timeline from engine-shaped add_clip events (--clips spread over 6 tracks, back to back), then
streams --edits move_clip / trim_clip completes at ~60/s (each one re-renders the screen), all measured in the page:
  - app frame work: time inside each of the app's rAF callbacks (p50 / p95 / max) and frame intervals while streaming
  - JS heap (performance.memory.usedJSHeapSize; Chromium only) after the build and after the edits
  - DOM: total element count and how many .clip / .crun elements the timeline panel holds
Exit 0 when frame work p95 <= 16.7 ms and the timeline holds at most a few elements per pixel column.

    python3 crates/kimo-gui/tests/timeline_perf_test.py [--clips 1000] [--edits 240]
"""
import argparse, asyncio, functools, http.server, json, random, socketserver, sys, threading
from pathlib import Path
CRATE = Path(__file__).resolve().parents[1]
ap = argparse.ArgumentParser(); ap.add_argument('--clips', type=int, default=1000); ap.add_argument('--edits', type=int, default=240); A = ap.parse_args()
PROBE = """(() => { const rAF = window.requestAnimationFrame.bind(window); window.__app = [];
  window.requestAnimationFrame = cb => rAF(t => { const a = performance.now(); cb(t); window.__app.push([t, performance.now() - a]); }); })()"""
class Q(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a): pass
def pct(xs, p): xs = sorted(xs); return xs[min(len(xs) - 1, int(round(p / 100 * (len(xs) - 1))))] if xs else 0

def build():
    evs = [{'type': 'session', 'width': 1920, 'height': 1080}]
    rnd = random.Random(3); ends = [0.0] * 6
    for k in range(A.clips):
        t = k % 6; tid = f'V{t + 1}'; s = ends[t]; d = round(rnd.uniform(.6, 4.0), 2); ends[t] = round(s + d, 2); cid = f'c{k + 1}'
        name = f'shot_{k + 1:04d}.mp4'
        evs.append({'tool_name': 'add_clip', 'call_id': f'a{k}', 'phase': 'start', 'target': {'track_id': tid, 'clip_id': cid}, 'payload': {'args': {'track_id': tid, 'path': f'/m/{name}', 'start': s}}})
        evs.append({'tool_name': 'add_clip', 'call_id': f'a{k}', 'phase': 'complete', 'target': {'track_id': tid, 'clip_id': cid}, 'payload': {'dur_ms': 1.2, 'result': {'clip_id': cid}, 'clip': cid, 'track': tid, 's': s, 'e': s + d, 'path': f'/m/{name}', 'changed': {'clips_added': [{'clip_id': cid, 'track_id': tid}]}}})
    return evs, ends

async def main():
    from playwright.async_api import async_playwright
    srv = socketserver.ThreadingTCPServer(('127.0.0.1', 0), functools.partial(Q, directory=str(CRATE))); srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    evs, ends = build(); rnd = random.Random(9)
    async with async_playwright() as p:
        b = await p.chromium.launch(args=['--enable-precise-memory-info']); pg = await b.new_page(viewport={'width': 1376, 'height': 768})
        errs = []; pg.on('pageerror', lambda e: errs.append(str(e)))
        await pg.add_init_script(PROBE)
        await pg.goto(f'http://127.0.0.1:{srv.server_address[1]}/ui/index.html?feed=off'); await pg.wait_for_timeout(500)
        import time; t0 = time.time()
        for i in range(0, len(evs), 200):
            await pg.evaluate('es => es.forEach(e => __kimo.apply(e))', evs[i:i + 200]); await pg.wait_for_timeout(16)
        await pg.wait_for_timeout(1200); build_s = time.time() - t0
        snap = """() => { const tl = document.querySelector('[data-panel=tl]') || document.querySelector('.k-timeline'); const tc = tl ? tl.querySelectorAll('.clip:not(.crun)').length : -1, tr = tl ? tl.querySelectorAll('.crun').length : -1;
          return { heap: performance.memory ? performance.memory.usedJSHeapSize : null, dom: document.getElementsByTagName('*').length, clips: tc, runs: tr, w: tl ? tl.clientWidth : 0, mode: __kimo.state.mode,
                   inState: (__kimo.state.liveTL ? __kimo.state.liveTL.tracks : []).reduce((n, t) => n + (t.clips || []).length, 0) }; }"""
        s1 = await pg.evaluate(snap)
        await pg.evaluate('() => { window.__app = []; }')
        for k in range(A.edits):
            c = rnd.randint(1, A.clips); cid = f'c{c}'; tid = f'V{(c - 1) % 6 + 1}'
            if k % 2:
                ev = {'tool_name': 'move_clip', 'call_id': f'm{k}', 'phase': 'complete', 'target': {'track_id': tid, 'clip_id': cid}, 'payload': {'dur_ms': 1.0, 'result': {'ok': True}, 'param': 'start', 'from': 0, 'to': 0.5, 's': 0.5, 'e': 1.5}}
            else:
                ev = {'tool_name': 'trim_clip', 'call_id': f'm{k}', 'phase': 'complete', 'target': {'track_id': tid, 'clip_id': cid}, 'payload': {'dur_ms': 1.0, 'result': {'ok': True}, 'param': 'duration', 'from': 2, 'to': 1.5}}
            st = dict(ev); st['phase'] = 'start'; st['payload'] = {'args': {'clip_id': cid}}
            await pg.evaluate('es => es.forEach(e => __kimo.apply(e))', [st, ev]); await pg.wait_for_timeout(16)
        await pg.wait_for_timeout(600)
        app = await pg.evaluate('window.__app'); s2 = await pg.evaluate(snap)
        work = [w for _, w in app]; ts = [t for t, _ in app]; ints = [y - x for x, y in zip(ts, ts[1:]) if y - x < 200]
        mb = lambda x: f'{x / 1048576:.1f} MB' if x else 'n/a'
        print(f"timeline: {s1['inState']} clips in state on 6 tracks ({max(ends):.0f} s), mode {s1['mode']}, built in {build_s:.1f} s")
        print(f"DOM after build: {s1['dom']} elements; timeline panel {s1['clips']} .clip + {s1['runs']} .crun for {s1['w']} px width")
        print(f"streaming {A.edits} edits at ~60/s: {len(work)} app frames; frame work p50 {pct(work, 50):.2f} / p95 {pct(work, 95):.2f} / max {max(work or [0]):.2f} ms; frame interval p50 {pct(ints, 50):.1f} / p95 {pct(ints, 95):.1f} ms")
        print(f"JS heap: {mb(s1['heap'])} after build, {mb(s2['heap'])} after edits; DOM after edits {s2['dom']} elements ({s2['clips']} .clip + {s2['runs']} .crun)")
        print('page errors', errs[:2]); await b.close()
    srv.shutdown()
    ok = pct(work, 95) <= 16.7 and s2['clips'] + max(s2['runs'], 0) <= 6 * max(1, s2['w']) // 2 and not errs
    print('RESULT:', 'within frame budget' if ok else 'FAIL'); return 0 if ok else 1
if __name__ == '__main__':
    sys.exit(asyncio.run(main()))
