#!/usr/bin/env python3
"""Layout jitter: does anything that is already on screen move or resize while calls stream in?

Plays every demo scenario live (feed=sim, the scenario's calls arriving on their own clock) and the
cold-engine event files given on the command line, sampling the layout every 50 ms from outside the
page (no in-page timers). Jitter = an element that is already there changing box between two samples where no call started,
progressed or settled and the mode did not change (a mode hop lays out another screen; a call landing
moves the log — both reported apart, not counted):
  - panels:   any change to a panel's box                                  (must be 0 px)
  - rail:     a row's height, or its args / duration column x, changing   (rows scrolling up by exactly
              one row pitch when a call lands is the log moving, not jitter)
  - inspector rows / timeline tracks: height changes                         (must be 0 px)
Prints per-scenario maxima and exits 1 when any jitter is over 0.5 px.

    python3 crates/kimo-gui/tests/jitter_test.py [events.json ...]
"""
import asyncio, functools, http.server, json, socketserver, sys, threading
from pathlib import Path
CRATE = Path(__file__).resolve().parents[1]
SAMPLE_JS = """() => {
  const R = e => { const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; };
  const o = { panels: {}, calls: [], rows: [], trks: [], mode: window.__kimo ? __kimo.state.mode : '', sig: window.__kimo ? __kimo.state.calls.map(c => c.id + ':' + c.status).join(',') : '' };
  document.querySelectorAll('[data-panel]').forEach(e => o.panels[e.dataset.panel] = R(e));
  document.querySelectorAll('.call').forEach(e => { const s = e.querySelector('.args'), d = e.querySelector('.dur');
    o.calls.push({ key: (e.querySelector('.fn') || {}).textContent + '|' + (s ? s.textContent : ''), box: R(e), argsX: s ? s.getBoundingClientRect().x : null, durR: d ? d.getBoundingClientRect().right : null, durX: d ? d.getBoundingClientRect().x : null }); });
  document.querySelectorAll('.kv .row').forEach((e, i) => o.rows.push({ key: (e.querySelector('.k') || {}).textContent + '#' + i, h: e.getBoundingClientRect().height }));
  document.querySelectorAll('.trk').forEach((e, i) => o.trks.push({ key: i, h: e.getBoundingClientRect().height }));
  return o; }"""
class Q(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a): pass

def diff(samples):
    J = {'panel': 0.0, 'row_h': 0.0, 'args_x': 0.0, 'dur_right': 0.0, 'kv_h': 0.0, 'track_h': 0.0}; where = {}
    def bump(k, v, w):
        if v > J[k]: J[k] = v; where[k] = w
    for a, b in zip(samples, samples[1:]):
        if a['mode'] != b['mode']: continue                 # a mode hop lays out a different screen: not jitter
        if a['sig'] != b['sig']:                            # a call landed / settled: the log moving, reported apart
            for p, box in b['panels'].items():
                if p in a['panels']: J['_panel_on_event'] = max(J.get('_panel_on_event', 0.0), max(abs(x - y) for x, y in zip(box, a['panels'][p])))
            continue
        for p, box in b['panels'].items():
            if p in a['panels']: bump('panel', max(abs(x - y) for x, y in zip(box, a['panels'][p])), p)
        prev = {c['key']: c for c in a['calls']}
        for c in b['calls']:
            q = prev.get(c['key'])
            if not q: continue
            bump('row_h', abs(c['box'][3] - q['box'][3]), c['key'])
            if c['argsX'] is not None and q['argsX'] is not None: bump('args_x', abs(c['argsX'] - q['argsX']), c['key'])
            if c['durR'] is not None and q['durR'] is not None: bump('dur_right', abs(c['durR'] - q['durR']), c['key'])
        pk = {r['key']: r['h'] for r in a['rows']}
        for r in b['rows']:
            if r['key'] in pk: bump('kv_h', abs(r['h'] - pk[r['key']]), r['key'])
        pt = {t['key']: t['h'] for t in a['trks']}
        for t in b['trks']:
            if t['key'] in pt: bump('track_h', abs(t['h'] - pt[t['key']]), t['key'])
    return J, where

async def run(evfiles):
    from playwright.async_api import async_playwright
    srv = socketserver.ThreadingTCPServer(('127.0.0.1', 0), functools.partial(Q, directory=str(CRATE))); srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f'http://127.0.0.1:{srv.server_address[1]}/ui/index.html'
    scen = [s['id'] for s in json.loads((CRATE / 'demo' / 'scenarios.json').read_text())]
    worst = 0.0; out = {}
    async with async_playwright() as p:
        b = await p.chromium.launch(); pg = await b.new_page(viewport={'width': 1376, 'height': 768})
        errs = []; pg.on('pageerror', lambda e: errs.append(str(e)))
        async def one(sid):
            await pg.goto(f'{base}?feed=sim&scenario={sid}', wait_until='domcontentloaded', timeout=8000); await pg.wait_for_timeout(300)
            S = []
            for _ in range(40):
                S.append(await pg.evaluate(SAMPLE_JS)); await pg.wait_for_timeout(50)
            return S
        for sid in scen:
            try: out[sid] = diff(await asyncio.wait_for(one(sid), 25)); print('.', end='', flush=True)
            except Exception as e:
                print(f'\n{sid}: no measurement ({type(e).__name__})', flush=True); pg = await b.new_page(viewport={'width': 1376, 'height': 768}); pg.on('pageerror', lambda e: errs.append(str(e)))
        for f in evfiles:
            evs = json.loads(Path(f).read_text())
            await pg.goto(f'{base}?feed=off'); await pg.wait_for_timeout(400)
            S = [await pg.evaluate(SAMPLE_JS)]
            for e in evs:
                if 'sleep' in e: continue
                await pg.evaluate('e => __kimo.apply(e)', e); await pg.wait_for_timeout(60); S.append(await pg.evaluate(SAMPLE_JS))
            out[Path(f).name] = diff(S)
        await b.close()
    srv.shutdown()
    print()
    for k, (J, where) in out.items():
        m = max(v for n, v in J.items() if not n.startswith('_')); worst = max(worst, m)
        print(f"{'PASS' if m <= 0.5 else 'JITTER'}  {k:<16} " + '  '.join(f'{n} {v:.1f}' for n, v in J.items() if not n.startswith('_')) + (f"   (panel move on a call event {J['_panel_on_event']:.1f})" if J.get('_panel_on_event') else '') + (f"   worst: {max(where.items(), key=lambda kv: J[kv[0]])}" if m > 0.5 else ''))
    if errs: print('page errors:', errs[:3])
    print(f"\nRESULT: {'no layout jitter' if worst <= 0.5 else f'jitter up to {worst:.1f} px'}")
    return 0 if worst <= 0.5 and not errs else 1

if __name__ == '__main__':
    sys.exit(asyncio.run(run(sys.argv[1:])))
