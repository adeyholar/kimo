#!/usr/bin/env python3
"""Router test: every tool call must be visible.

Loads the m16 edit screen (still snapshot), then injects unified tool events
(the engine contract in README "Unified tool events"): one per tool family
plus an unknown tool and an error. For each call it asserts:
  - the agent rail gained an entry with the tool's name (and an error state
    for the failing call),
  - the DOM changed on start and again on completion,
  - the family's own effect landed (clip split / added, fx badge, track
    header state, inspector diff, caption chip, HUD pulse...).
Finally it checks the render loop goes idle again (0 rAF over 3 s).

    python3 crates/kimo-gui/tests/router_test.py      # exit 1 on failure
"""
import asyncio, functools, http.server, json, socketserver, sys, threading
from pathlib import Path
CRATE = Path(__file__).resolve().parents[1]

PROBE = r"""
(() => {
  window.__muts = 0; window.__raf = 0;
  const rAF = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = cb => rAF(t => { window.__raf++; cb(t); });
  addEventListener('DOMContentLoaded', () => new MutationObserver(m => { window.__muts += m.length; })
    .observe(document.getElementById('main'), { subtree: true, childList: true, attributes: true, characterData: true }));
})();
"""

EVENTS = [
  ('ingest',    'import_media',  {'track_id': 'V2'}, {'track': 'V2', 's': 18.5, 'e': 21.5, 'label': 'new_clip', 'clip': 'new_clip'},
               "[...document.querySelectorAll('[data-panel=tl] .clip .cl')].some(e => e.textContent === 'new_clip')"),
  ('edit',      'split_clip',    {'track_id': 'V1', 'clip_id': 'dune_wide'}, {'at': 11},
               "document.querySelectorAll('[data-panel=tl] .trk')[1].querySelectorAll('.clip').length === 4"),
  ('effects',   'apply_effect',  {'track_id': 'V1', 'clip_id': 'dune_ridge'}, {'effect': 'glow'},
               "[...document.querySelectorAll('[data-panel=tl] .fxchip')].some(e => e.textContent === 'glow')"),
  ('transform', 'set_transform', {'param': 'Scale'}, {'from': '104%', 'to': '110%'},
               "[...document.querySelectorAll('[data-panel=insp] .row')].some(r => r.textContent.includes('110%') && r.textContent.includes('104%'))"),
  ('transform', 'set_property',  {'clip_id': 'clip4', 'param': 'scale'}, {'from': 1.04, 'to': 1.12},
               "[...document.querySelectorAll('[data-panel=agent] .call')].some(r => r.textContent.includes('scale: 104% → 112%'))"),
  ('color',     'grade',         {'clip_id': 'clip_04', 'param': 'Temperature'}, {'from': '6050K', 'to': '6200K'},
               "[...document.querySelectorAll('[data-panel=insp] .row')].some(r => r.textContent.includes('6200K'))"),
  ('audio',     'mute_track',    {'track_id': 'A2'}, {'on': True},
               "!!document.querySelector('[data-panel=tl] .trk.muted .ms .m')"),
  ('text',      'add_caption',   {'track_id': 'CAP'}, {'text': 'Again.', 's': 19.2, 'e': 21.5},
               "[...document.querySelectorAll('[data-panel=tl] .clip.cap .cl')].some(e => e.textContent === 'Again.')"),
  ('3d',        'add_light',     {}, {'intensity': 2.4},
               "[...document.querySelectorAll('[data-panel=insp] .row')].some(r => r.textContent.includes('2.4'))"),
  ('export',    'export_render', {}, {'preset': 'h264_master'},
               "[...document.querySelectorAll('[data-panel=insp] .row')].some(r => r.textContent.includes('h264_master'))"),
  ('other',     'frobnicate_widget', {}, {'knob': 7},
               "[...document.querySelectorAll('[data-panel=insp] .row')].some(r => r.textContent.includes('Knob'))"),
]

class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a): pass

async def main():
    from playwright.async_api import async_playwright
    srv = socketserver.ThreadingTCPServer(('127.0.0.1', 0), functools.partial(Quiet, directory=str(CRATE))); srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    url = f'http://127.0.0.1:{srv.server_address[1]}/ui/index.html?feed=sim&scenario=m16&still=1'
    fails, rows = [], []
    async with async_playwright() as p:
        b = await p.chromium.launch()
        ctx = await b.new_context(viewport={'width': 1376, 'height': 768}); await ctx.add_init_script(PROBE)
        pg = await ctx.new_page(); errs = []
        pg.on('pageerror', lambda e: errs.append(str(e)))
        await pg.goto(url); await pg.wait_for_function('window.__kimo && __kimo.metrics.renders > 0'); await pg.wait_for_timeout(400)
        frame = lambda: pg.wait_for_timeout(60)
        BAD = r"""() => { const re = /\b(undefined|NaN|null)\b/; return [...document.querySelectorAll('[data-panel=agent] .call, [data-panel=insp] .row, [data-panel=insp] .sec')].map(e => e.textContent).filter(s => re.test(s)); }"""
        rail = "() => [...document.querySelectorAll('[data-panel=agent] .call .fn')].map(e => e.textContent)"
        for k, (fam, name, target, payload, check) in enumerate(EVENTS + [('error', 'blade_cut', {'track_id': 'V9', 'clip_id': 'nope'}, {'error': 'no such clip'}, None)]):
            cid = f't{k}'
            m0 = await pg.evaluate('__muts')
            await pg.evaluate('e => __kimo.apply(e)', {'tool_name': name, 'call_id': cid, 'phase': 'start', 'target': target, 'payload': {}, 'ts': k})
            await frame()
            on_start = (await pg.evaluate('__muts')) > m0 and name in (await pg.evaluate(rail))
            await pg.evaluate('e => __kimo.apply(e)', {'tool_name': name, 'call_id': cid, 'phase': 'progress', 'target': target, 'payload': {'pct': 50}, 'ts': k})
            m1 = await pg.evaluate('__muts')
            phase = 'error' if fam == 'error' else 'complete'
            await pg.evaluate('e => __kimo.apply(e)', {'tool_name': name, 'call_id': cid, 'phase': phase, 'target': target, 'payload': {**payload, 'dur_ms': 120 + k}, 'ts': k})
            await frame()
            on_end = (await pg.evaluate('__muts')) > m1
            hud = await pg.evaluate("document.querySelector('[data-panel=viewer] .hud') ? document.querySelector('[data-panel=viewer] .hud').textContent : ''")
            if fam == 'error':
                effect = await pg.evaluate("!!document.querySelector('[data-panel=agent] .call.err')")
            else:
                effect = await pg.evaluate(check)
            bad = await pg.evaluate(BAD)
            if bad: print('   bad text on rail/inspector:', bad[:3])
            ok = on_start and on_end and effect and name in hud and not bad
            rows.append((fam, name, on_start, on_end, effect, hud))
            print(f"{'PASS' if ok else 'FAIL'}  {fam:<9} {name:<18} rail+DOM on start={on_start!s:<5} DOM on end={on_end!s:<5} effect={effect!s:<5} hud='{hud}'", flush=True)
            if not ok: fails.append(name)
        await pg.wait_for_timeout(2000)
        r0 = await pg.evaluate('__raf'); await pg.wait_for_timeout(3000); r1 = await pg.evaluate('__raf')
        idle = r1 == r0
        print(f"{'PASS' if idle else 'FAIL'}  idle after the burst: {r1 - r0} rAF in 3 s")
        if not idle: fails.append('idle')
        if errs: print('page errors:', errs); fails.append('pageerror')
        await b.close()
    srv.shutdown()
    print('\nRESULT:', 'every tool call visible' if not fails else 'FAILED: ' + ', '.join(fails))
    sys.exit(1 if fails else 0)

asyncio.run(main())
