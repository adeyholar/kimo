#!/usr/bin/env python3
"""COV-A reactive test: the thirteen timeline tools that used to hit the generic fallback.

Replays real kimo-mcp captures (tests/ev_cov_a.json, three engine sessions) into a cold page
(?feed=off) and, after each COV-A tool's completion, asserts the tool's OWN change on the screen
the call lands on: not the rail row or HUD pulse every call gets, but the marker tick on the ruler,
the beat ticks on the clip, the key diamonds and pulse curve, the badge, the moved clip, the
multicam spans, the viewer tag, the Inspector rows with the call's values. Every touched row is
checked for raw ids (clip4, track10, marker7, asset2...) and undefined/NaN/null.

    python3 crates/kimo-gui/tests/cov_a_test.py      # exit 1 on failure
"""
import asyncio, functools, http.server, json, socketserver, sys, threading
from pathlib import Path
CRATE = Path(__file__).resolve().parents[1]
EV = json.loads(Path(__file__).with_name('ev_cov_a.json').read_text())['sessions']

class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a): pass

# helpers in the page: rows of the Inspector-kind panel on screen, the tl clips, viewer tags
JS = r"""
window.__q = {
  mode: () => __kimo.state.mode,
  rows: () => [...document.querySelectorAll('.kv .row')].map(r => r.textContent.replace(/\s+/g, ' ').trim()),
  secs: () => [...document.querySelectorAll('.kv .sec')].map(r => r.textContent.trim()),
  tags: () => [...document.querySelectorAll('.frame .tag')].map(e => e.textContent),
  clips: () => [...document.querySelectorAll('.trk .clip')].map(c => ({ label: (c.querySelector('.cl') || {}).textContent || '', left: parseFloat(c.style.left), width: parseFloat(c.style.width),
      fx: [...c.querySelectorAll('.fxchip')].map(e => e.textContent), beats: c.querySelectorAll('.cbeat').length, keys: c.querySelectorAll('.ckey.em').length })),
  marks: () => ({ live: document.querySelectorAll('.tlmk:not(.gone)').length, gone: document.querySelectorAll('.tlmk.gone').length, rng: document.querySelectorAll('.tlmk.rng:not(.gone)').length,
      cuts: document.querySelectorAll('.tlcut').length, razors: document.querySelectorAll('.trk ~ .razor, .tl > .razor').length }),
  graph: () => { const g = document.querySelector('[data-panel=graph]'); if (!g) return null;
      return { lanes: [...g.querySelectorAll('.labchip')].map(e => e.textContent), diamonds: g.querySelectorAll('svg path[fill="#d9d4c9"]').length,
               ember: g.querySelectorAll('svg path[stroke="var(--ember)"]').length, keys: g.querySelectorAll('svg rect[fill="var(--ember)"]').length }; },
};
"""
RAW = r"\b(clip|track|marker|asset|multicam|captions)\d+\b"

# tool -> list of (description, JS predicate) checked after its Nth completion (index by occurrence)
CHECKS = {
  0: {  # session 1
    ('add_marker', 0): [("ember tick on the ruler", "__q.marks().live === 1 && __q.marks().rng === 0"),
                        ("Inspector names it", "__q.rows().some(r => r.includes('Marker') && r.includes('drop')) && __q.rows().some(r => r.includes('At') && r.includes('1.5 s'))"),
                        ("tick sits at 1.5 s on the ruler", "(() => { const m = document.querySelector('.tlmk'); const lab = [...document.querySelectorAll('.ruler span')]; return !!m && parseFloat(m.style.left) > 57; })()")],
    ('add_marker', 1): [("range marker draws a band", "__q.marks().live === 2 && __q.marks().rng === 1 && parseFloat(document.querySelector('.tlmk.rng').style.width) > 4"),
                        ("Inspector length", "__q.rows().some(r => r.includes('Length') && r.includes('0.5 s'))")],
    ('remove_marker', 0): [("the drop marker fades out, the range stays", "__q.marks().gone === 1 && __q.marks().live === 1 && __q.marks().rng === 1"),
                           ("Inspector from -> removed", "__q.rows().some(r => r.includes('drop · 1.5 s') && r.includes('removed'))")],
    ('detect_beats', 0): [("on the mixing screen", "__q.mode() === 'mixing'"),
                          ("7 beat ticks on cam_a.mp4", "__q.clips().some(c => c.label === 'cam_a.mp4' && c.beats === 7)"),
                          ("tempo on the Audio rows", "__q.rows().some(r => r.includes('Tempo') && r.includes('120.1 bpm')) && __q.rows().some(r => r.includes('Beats') && r.includes('7'))")],
    ('detect_cuts', 0): [("edit screen", "__q.mode() === 'edit'"), ("none found", "__q.rows().some(r => r.includes('Cuts') && r.includes('none found')) && __q.marks().cuts === 0")],
    ('detect_cuts', 1): [("one cut tick through the tracks", "__q.marks().cuts === 1"),
                         ("Inspector hard cut at 2 s on cam_a.mp4", "__q.rows().some(r => r.includes('Hard cut') && r.includes('2 s')) && __q.rows().some(r => r.includes('Source') && r.includes('cam_a.mp4'))")],
    ('bind_beats', 0): [("on the Graph screen", "__q.mode() === 'graph'"),
                        ("Graph lane names clip + property", "__q.graph().lanes.includes('cam_a.mp4 · scale')"),
                        ("8 beat diamonds, an ember pulse curve with 8 hit keys", "__q.graph().diamonds === 8 && __q.graph().ember >= 1 && __q.graph().keys === 8"),
                        ("Inspector hit", "__q.rows().some(r => r.includes('Hit') && r.includes('+10%') && r.includes('pulse')) && __q.rows().some(r => r.includes('Beats') && r.includes('8'))")],
    ('set_time_mode', 0): [("reverse badge on cam_b.mp4", "__q.mode() === 'edit' && __q.clips().some(c => c.label === 'cam_b.mp4' && c.fx.includes('reverse'))"),
                           ("8 key diamonds left on cam_a.mp4 by bind_beats", "__q.clips().some(c => c.label === 'cam_a.mp4' && c.keys === 8)"),
                           ("Inspector normal -> reverse", "__q.rows().some(r => r.includes('Time') && r.includes('normal') && r.includes('reverse'))")],
    ('set_time_mode', 1): [("freeze badge replaces reverse", "__q.clips().some(c => c.label === 'cam_b.mp4' && c.fx.includes('freeze 1.25 s') && !c.fx.includes('reverse'))")],
    ('set_time_mode', 2): [("badge gone on normal", "__q.clips().some(c => c.label === 'cam_b.mp4' && !c.fx.some(f => /reverse|freeze/.test(f)))"),
                           ("Inspector freeze -> normal", "__q.rows().some(r => r.includes('freeze 1.25 s') && r.includes('normal'))")],
    ('sync_clips', 0): [("cam_b.mp4 slides right by 0.5 s", "(() => { const c = __q.clips(), a = c.find(x => x.label === 'cam_a.mp4'), b = c.find(x => x.label === 'cam_b.mp4'); return a && b && b.left - a.left > 4; })()"),
                        ("Inspector 0 s -> 0.5 s", "__q.rows().some(r => r.includes('cam_b.mp4') && r.includes('0 s') && r.includes('0.5 s')) && __q.rows().some(r => r.includes('By') && r.includes('timecode'))")],
    ('add_multicam', 0): [("sources move into one group clip", "(() => { const L = __q.clips().map(c => c.label); return L.includes('cam_a.mp4 · angle 1') && !L.includes('cam_a.mp4') && !L.includes('cam_b.mp4'); })()"),
                          ("multicam badge tag on the viewer", "__q.tags().includes('multicam · 2 angles')"),
                          ("Inspector angles", "__q.rows().some(r => r.includes('Angles') && r.includes('cam_a.mp4, cam_b.mp4'))")],
    ('switch_angle', 0): [("spans: angle 1 then angle 2", "(() => { const L = __q.clips().map(c => c.label).filter(l => l.includes('angle')); return JSON.stringify(L) === JSON.stringify(['cam_a.mp4 · angle 1', 'cam_b.mp4 · angle 2']); })()"),
                          ("viewer tag", "__q.tags().includes('angle 2 · cam_b.mp4')"),
                          ("Inspector from -> to", "__q.rows().some(r => r.includes('Angle') && r.includes('1 · cam_a.mp4') && r.includes('2 · cam_b.mp4'))")],
    ('switch_angle', 1): [("three spans", "(() => { const L = __q.clips().map(c => c.label).filter(l => l.includes('angle')); return JSON.stringify(L) === JSON.stringify(['cam_a.mp4 · angle 1', 'cam_b.mp4 · angle 2', 'cam_a.mp4 · angle 1']); })()")],
    ('match_events', 0): [("Inspector match rows", "__q.rows().some(r => r.includes('Matched') && r.includes('4 of 4')) && __q.rows().some(r => r.includes('Mean offset') && r.includes('−0.06 s')) && __q.rows().some(r => r.includes('Max offset') && r.includes('0.4 s'))")],
    ('remove_silence_cuts', 0): [("Inspector removed", "__q.rows().some(r => r.includes('Removed') && r.includes('2 ranges') && r.includes('0.75 s'))")],
  },
  1: {  # session 2
    ('burn_in', 0): [("captions screen", "__q.mode() === 'captions'"),
                     ("caption clip says burn-in off", "__q.clips().some(c => c.label === 'hello there' && c.fx.includes('burn-in off'))"),
                     ("viewer tag", "__q.tags().includes('captions off in preview + export')"),
                     ("Inspector on -> off", "__q.rows().some(r => r.includes('Burn-in') && r.includes('on') && r.includes('off'))")],
    ('burn_in', 1): [("badge cleared", "__q.clips().some(c => c.label === 'hello there' && !c.fx.includes('burn-in off'))"), ("viewer tag", "__q.tags().includes('captions burned in')")],
    ('transition_render_raw', 0): [("effects screen + viewer tag", "__q.mode() === 'effects' && __q.tags().includes('crossfade · 40%')"),
                                   ("Inspector rows", "__q.rows().some(r => r.includes('Frame') && r.includes('2 x 2')) && __q.rows().some(r => r.includes('Edge') && r.includes('clamp'))")],
  },
  2: {  # session 3: clip at 1 s
    ('detect_cuts', 0): [("clip source: cut at 3 s on the timeline", "__q.marks().cuts === 1 && __q.rows().some(r => r.includes('Hard cut') && r.includes('3 s'))")],
    ('detect_cuts', 1): [("timeline source: two cuts", "__q.marks().cuts === 2")],
    ('detect_cuts', 2): [("file source maps onto the clip (2 s + 1 s)", "__q.marks().cuts === 1 && __q.rows().some(r => r.includes('Hard cut') && r.includes('3 s')) && __q.rows().some(r => r.includes('Source') && r.includes('cam_a.mp4'))")],
    ('remove_silence_cuts', 0): [("the clip splits around the removed range, razor at the joint", "(() => { const c = __q.clips().filter(x => x.label === 'cam_a.mp4'); return c.length === 2 && __q.marks().razors >= 1; })()"),
                                 ("Inspector", "__q.rows().some(r => r.includes('Removed') && r.includes('1 range ·') && r.includes('0.5 s')) && __q.rows().some(r => r.includes('Track') && r.includes('V1'))")],
  },
}
COV = set("add_marker remove_marker add_multicam switch_angle sync_clips detect_cuts detect_beats bind_beats remove_silence_cuts set_time_mode match_events transition_render_raw burn_in".split())

async def main():
    from playwright.async_api import async_playwright
    srv = socketserver.ThreadingTCPServer(('127.0.0.1', 0), functools.partial(Quiet, directory=str(CRATE))); srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    url = f'http://127.0.0.1:{srv.server_address[1]}/ui/index.html?feed=off'
    fails, seen, ok, ran = [], set(), 0, set()
    async with async_playwright() as p:
        b = await p.chromium.launch()
        try:
            for si, events in enumerate(EV):
                ctx = await b.new_context(viewport={'width': 1376, 'height': 768}); await ctx.add_init_script(JS)
                pg = await ctx.new_page(); errs = []
                pg.on('pageerror', lambda e: errs.append(str(e)))
                await pg.goto(url); await pg.wait_for_function('window.__kimo && __kimo.metrics.renders > 0')
                count = {}
                for ev in events:
                    tn = ev.get('tool_name')
                    snap = None
                    if tn in COV and ev.get('phase') == 'complete':
                        snap = await pg.evaluate("() => ({ rows: __q.rows().join('|'), clips: JSON.stringify(__q.clips()), marks: JSON.stringify(__q.marks()), tags: __q.tags().join('|'), graph: JSON.stringify(__q.graph()) })")
                    await pg.evaluate('e => __kimo.apply(e)', ev)
                    if snap is None: continue
                    await pg.wait_for_timeout(70)
                    k = count.get(tn, 0); count[tn] = k + 1; seen.add(tn)
                    after = await pg.evaluate("() => ({ rows: __q.rows().join('|'), clips: JSON.stringify(__q.clips()), marks: JSON.stringify(__q.marks()), tags: __q.tags().join('|'), graph: JSON.stringify(__q.graph()) })")
                    if after == snap: fails.append(f's{si} {tn}#{k}: nothing on screen changed')
                    bad = await pg.evaluate(f"""() => {{ const re = /{RAW}|\\b(undefined|NaN|null)\\b/; return [...__q.rows(), ...__q.secs().filter(s => !/^[a-z_]+$/.test(s)), ...__q.tags(), ...__q.clips().map(c => c.label + ' ' + c.fx.join(' '))].filter(s => re.test(s)); }}""")
                    if bad: fails.append(f's{si} {tn}#{k}: raw id / undefined on screen: {bad[:3]}')
                    ran.add((si, tn, k))
                    for desc, js in CHECKS.get(si, {}).get((tn, k), []):
                        try: res = await pg.evaluate(js)
                        except Exception as e: res = f'error {e}'
                        if res is True: ok += 1; print(f'PASS  s{si} {tn}#{k}: {desc}')
                        else:
                            fails.append(f's{si} {tn}#{k}: {desc}')
                            print(f'FAIL  s{si} {tn}#{k}: {desc}\n      rows={await pg.evaluate("__q.rows()")}\n      clips={await pg.evaluate("__q.clips()")}\n      marks={await pg.evaluate("__q.marks()")} tags={await pg.evaluate("__q.tags()")} mode={await pg.evaluate("__q.mode()")} graph={await pg.evaluate("__q.graph()")}')
                rail = await pg.evaluate(f"() => __kimo.state.calls.filter(c => /^({'|'.join(sorted(COV))})$/.test(c.name)).map(c => c.name + ' ' + JSON.stringify(c.args || {{}}))")
                badr = [r for r in rail if __import__('re').search(RAW + r'|undefined|NaN|"\s*→\s*,', r)]
                if badr: fails.append(f's{si}: rail args carry raw ids / junk: {badr[:3]}')
                else: ok += 1; print(f'PASS  s{si} rail: {len(rail)} COV-A calls, args read as names')
                if errs: fails.append(f's{si}: page errors {errs[:3]}')
                await ctx.close()
        finally:
            await b.close(); srv.shutdown()
    unrun = [f's{si} {tn}#{k}' for si, c in CHECKS.items() for (tn, k) in c if (si, tn, k) not in ran]
    if unrun: fails.append(f'checks that never ran: {unrun}')
    missing = sorted(COV - seen)
    if missing: fails.append(f'tools never exercised: {missing}')
    print(f'\ncov_a: {ok} checks passed, {len(seen)}/13 tools exercised, {len(fails)} failures')
    for f in fails: print('  -', f)
    sys.exit(1 if fails else 0)

asyncio.run(main())
