#!/usr/bin/env python3
"""Spring-eased splits: frame times while panels spring between two screens, and 0 px for everything else.

For each mode hop below (an engine 'mode' event, the way a live session changes screen), a panel that exists on
both screens springs from its old box to its new one (transform only). Measured in the page with a passive
observer loop (test-only; the app itself runs rAF only while a spring is moving):
  - frame intervals while any panel is springing: p50 / p95 / max (60 fps = 16.7 ms) and the app's own work per frame
  - settle time (hop to last spring frame)
  - jitter: a panel that is NOT springing must not move by even 0.5 px on any frame of the hop
  - idle: after settling, 0 app rAF over 1 s
Exit 0 when p95 <= 20 ms, no jitter, idle 0.

    python3 crates/kimo-gui/tests/spring_test.py
"""
import asyncio, functools, http.server, json, socketserver, sys, threading
from pathlib import Path
CRATE = Path(__file__).resolve().parents[1]
HOPS = [('edit', 'effects'), ('effects', 'roto'), ('roto', 'graph'), ('graph', 'scene3d'), ('scene3d', 'lighting'), ('lighting', 'export'), ('export', 'edit'), ('edit', 'grading')]
PROBE = """(() => { const rAF = window.requestAnimationFrame.bind(window); window.__app = []; window.__appRaf = 0;
  window.requestAnimationFrame = cb => { window.__appRaf++; return rAF(t => { const a = performance.now(); cb(t); window.__app.push([t, performance.now() - a]); }); };
  window.__obs = null; window.__watch = n => { const S = window.__obs = []; let k = 0; const loop = t => { const o = {}; document.querySelectorAll('[data-panel]').forEach(e => { const r = e.getBoundingClientRect(); o[e.dataset.panel] = [r.x, r.y, r.width, r.height, e.dataset.anim ? 1 : 0]; }); S.push([t, o]); if (++k < n) rAF(loop); }; rAF(loop); }; })()"""
class Q(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a): pass
def pct(xs, p): xs = sorted(xs); return xs[min(len(xs) - 1, int(round(p / 100 * (len(xs) - 1))))] if xs else 0

async def main():
    from playwright.async_api import async_playwright
    srv = socketserver.ThreadingTCPServer(('127.0.0.1', 0), functools.partial(Q, directory=str(CRATE))); srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ok = True; allint = []; allwork = []
    async with async_playwright() as p:
        b = await p.chromium.launch(); pg = await b.new_page(viewport={'width': 1376, 'height': 768})
        errs = []; pg.on('pageerror', lambda e: errs.append(str(e)))
        await pg.add_init_script(PROBE)
        await pg.goto(f'http://127.0.0.1:{srv.server_address[1]}/ui/index.html?feed=off'); await pg.wait_for_timeout(500)
        await pg.evaluate("() => __kimo.apply({type: 'session', project: 'dune_teaser', agent: 'claude'})")
        for a, z in HOPS:
            await pg.evaluate(f"() => __kimo.apply({{type: 'mode', mode: '{a}'}})"); await pg.wait_for_timeout(900)
            await pg.evaluate('() => { window.__app = []; }')
            await pg.evaluate('n => __watch(n)', 50)
            await pg.evaluate(f"() => __kimo.apply({{type: 'mode', mode: '{z}'}})")
            await pg.wait_for_timeout(1100)
            obs = await pg.evaluate('window.__obs'); app = await pg.evaluate('window.__app')
            # frames while springing: observer frames where any panel carries data-anim
            sp = [t for t, o in obs if any(v[4] for v in o.values())]
            ints = [y - x for x, y in zip(sp, sp[1:])]
            work = [w for t, w in app]
            jit = 0.0; jw = ''; moved = set()
            for (t0, A), (t1, B) in zip(obs, obs[1:]):
                if set(A) != set(B): continue          # the hop itself swaps screens
                for k, v in B.items():
                    if v[4] or A[k][4]: moved.add(k); continue
                    d = max(abs(x - y) for x, y in zip(v[:4], A[k][:4]))
                    if d > jit: jit, jw = d, k
            settle = (sp[-1] - obs[0][0]) if sp else 0
            r0 = await pg.evaluate('window.__appRaf'); await pg.wait_for_timeout(1000); r1 = await pg.evaluate('window.__appRaf')
            allint += ints; allwork += work
            good = jit <= .5 and r1 == r0 and (not ints or pct(ints, 95) <= 20)
            ok &= good
            print(f"{'PASS' if good else 'FAIL'}  {a:>8} -> {z:<9} springing {sorted(moved) or '-'}  frames {len(sp)}  interval p50 {pct(ints, 50):.1f} p95 {pct(ints, 95):.1f} max {max(ints or [0]):.1f} ms  app work p95 {pct(work, 95):.2f} ms  settle {settle:.0f} ms  still-panel jitter {jit:.1f}px{(' (' + jw + ')') if jit > .5 else ''}  rAF after {r1 - r0}")
        await b.close()
    srv.shutdown()
    print(f"\nall hops: {len(allint)} spring frame intervals, p50 {pct(allint, 50):.1f} / p95 {pct(allint, 95):.1f} / max {max(allint or [0]):.1f} ms ({1000 / max(pct(allint, 50), 1e-6):.0f} fps median); app work per frame p95 {pct(allwork, 95):.2f} ms, max {max(allwork or [0]):.2f} ms; page errors {errs[:2]}")
    ok &= not errs
    print('RESULT:', 'springs at frame rate, no jitter outside them, idle after' if ok else 'FAIL'); return 0 if ok else 1
if __name__ == '__main__':
    sys.exit(asyncio.run(main()))
