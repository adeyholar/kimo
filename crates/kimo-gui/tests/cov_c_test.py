#!/usr/bin/env python3
"""COV-C reactive test: the colour-pipeline, project / history / template / cache and frame-QA tools.

Replays tests/ev_engine_cov_c.json (a real kimo-mcp capture: import, add_clip, checkpoint, the colour
pipeline, set_meta, project_diff, undo / redo, restore, checkpoint_delete, batch, templates, a recipe,
save / load, the frame cache, diff_frames, lint_frame) into a cold live page, plus two synthetic events the
engine on this box cannot produce (set_ocio_view_delivery needs the native-ocio build; lint findings carry
their rects only below the spectator's depth limit). For every one of the 22 COV-C tools it asserts:
  - the tool is classified dedicated by ui/router.js family() (not the generic fallback),
  - a TOOL-SPECIFIC element in an existing mockup panel is absent before the call lands and present after
    (Color pipeline row, History row, Diff line, Batch table, Media row / footer, timeline label + Inspector);
    the read / tune tools (diff_frames, lint_frame, cache_clear, cache_set_budget; Naman 7:24: HUD status toast,
    rail row) must put their RESULT in the HUD toast and on their rail row, without a mode hop or a canvas change
    -- a bare rail row or a bare "tool ✓" HUD pulse does not count,
  - no raw id (clip3, track1, asset2, clip1000...) and no undefined / NaN / null in those panels,
and that the page goes idle afterwards (0 rAF over 3 s).

    python3 crates/kimo-gui/tests/cov_c_test.py     # exit 1 on failure
"""
import asyncio, functools, http.server, json, re, socketserver, sys, threading
from pathlib import Path
CRATE = Path(__file__).resolve().parents[1]
EV = json.loads(Path(__file__).with_name('ev_engine_cov_c.json').read_text())
TOOLS = ['set_clip_input_color', 'set_color_pipeline', 'set_hdr_mastering', 'set_ocio_view_delivery', 'set_meta', 'load_project',
         'save_project', 'project_diff', 'checkpoint', 'checkpoint_delete', 'restore', 'undo', 'redo', 'batch', 'recipe_apply',
         'template_apply', 'template_define', 'template_delete', 'cache_clear', 'cache_set_budget', 'diff_frames', 'lint_frame']

PROBE = r"""
(() => {
  window.__raf = 0;
  const rAF = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = cb => rAF(t => { window.__raf++; cb(t); });
})();
"""
JS = r"""
window.T = sel => [...document.querySelectorAll(sel)].map(e => e.textContent.replace(/\s+/g, ' ').trim());
window.has = (sel, ...parts) => T(sel).some(s => parts.every(p => s.includes(p)));
window.hud = s => { const h = document.querySelector('[data-panel=viewer] .hud'); return !!h && h.textContent.includes(s); };
window.rail = (fn, s) => [...document.querySelectorAll('[data-panel=agent] .call')].some(r => r.textContent.includes(fn) && r.textContent.includes(s));
window.row = (k, v, from) => [...document.querySelectorAll('[data-panel=pipe] .row')].some(r => r.querySelector('.k').textContent === k && r.querySelector('.v').textContent.endsWith(v) && (from == null || ((r.querySelector('.from') || {}).textContent === from)));
"""
# per call: the predicate that must be false before the call completes and true after (in the panel the tool owns)
CHECK = {
  'checkpoint#1':        "has('[data-panel=hist] .hrow', 'before_grade', 'checkpoint', '●')",
  'set_color_pipeline':  "row('input', 'Rec.709 sRGB') && row('working', 'ACEScg') && row('output', 'Rec.2100 PQ')",
  'set_clip_input_color':"row('shot_0009', 'AP0 ACEScct') && !!document.querySelector('[data-panel=pipe] .row.flash')",
  'set_hdr_mastering':   "row('MaxCLL', '1000 nits') && row('MaxFALL', '400 nits') && row('Peak', '1000 nits') && T('[data-panel=pipe] .sec').includes('HDR')",
  'set_ocio_view_delivery#1': "row('view', '✗ ACES 1.0 SDR') && !!document.querySelector('[data-panel=pipe] .row.dim') && !document.querySelector('[data-panel=pipe] .row.emb')",
  'set_meta#1':          "T('[data-panel=tl] .clip .cl').includes('hero_wide') && has('[data-panel=insp] .row', 'Name', 'shot_0009.mp4', 'hero_wide') && has('[data-panel=insp] .row', 'Role', 'a-roll') && has('[data-panel=insp] .row', 'Tags', 'hero, day')",
  'set_meta#2':          "T('[data-panel=tl] .trk .lab').some(s => s.includes('dialogue'))",
  'checkpoint#2':        "has('[data-panel=hist] .hrow', 'graded', 'checkpoint', '●')",
  'project_diff':        "has('[data-panel=diff] .ph', 'before_grade to now') && has('[data-panel=diff] div', '~', 'hero_wide 5 changes') && has('[data-panel=diff] div', 'track unnamed -> dialogue')",
  'undo#1':              "has('[data-panel=hist] .hrow', 'undo', '9 left', '●') && hud('undo 1 step · 9 left to undo, 1 to redo')",
  'redo#1':              "has('[data-panel=hist] .hrow', 'redo', '10 left', '●') && !has('[data-panel=hist] .hrow', '9 left') && hud('redo 1 step · 10 left to undo, 0 to redo')",
  'undo#2':              "has('[data-panel=hist] .hrow', 'undo 2 steps', '8 left') && hud('undo 2 steps · 8 left to undo, 2 to redo')",
  'redo#2':              "has('[data-panel=hist] .hrow', 'redo 2 steps', '10 left') && hud('redo 2 steps')",
  'restore':             "has('[data-panel=hist] .hrow', 'restore', 'before_grade', '●') && hud('restore · back to before_grade') && [...document.querySelectorAll('[data-panel=hist] .hrow')].some(r => r.textContent.includes('before_grade') && r.textContent.includes('checkpoint') && r.querySelector('.thumb.sel'))",
  'checkpoint_delete':   "[...document.querySelectorAll('[data-panel=hist] .hrow')].some(r => r.textContent.includes('before_grade') && r.textContent.includes('deleted') && r.style.opacity === '0.45')",
  'batch':               "has('[data-panel=diff] div', 'Batch trim both', '2 / 2') && has('[data-panel=diff] div', 'hero_wide', 'trim_clip', 'warned') && has('[data-panel=diff] div', 'shot_0015.mp4', 'trim_clip')",
  'template_define':     "has('[data-panel=diff] div', '+', 'template lower_third (text, at) · 1 op')",
  'template_apply':      "has('[data-panel=diff] div', 'Template lower_third', '1 op') && has('[data-panel=diff] div', 'Jane Doe', 'lower_third', 'added')",
  'recipe_apply':        "has('[data-panel=diff] div', 'Recipe lower_third', '40 ops') && ['bar', 'accent', 'title', 'subtitle'].every(k => has('[data-panel=diff] div', k, 'lower_third', 'added'))",
  'template_delete':     "T('[data-panel=diff] div').some(s => s === '-template lower_third')",
  'save_project':        "has('[data-panel=hist] .hrow', 'proj.kimo', 'saved', '●') && hud('save_project · proj.kimo + history: 16 undo steps, 1 checkpoint') && has('[data-panel=media] .pfoot', 'save_project: proj.kimo, history')",
  'load_project':        "T('[data-panel=hist] .hrow').length === 1 && has('[data-panel=hist] .hrow', 'proj.kimo', 'loaded') && hud('load_project · proj.kimo: 16 undo steps, 1 checkpoint') && has('[data-panel=media] div', 'Montserrat', 'font missing ×3') && has('[data-panel=media] .pfoot', 'load_project: 5 files, 3 missing')",
  'cache_set_budget':    "hud('frame cache 512 MB, 0 B used') && rail('cache_set_budget', 'budget: 512 MB')",
  'cache_clear':         "hud('frame cache cleared · 0 frames, 0 B freed') && rail('cache_clear', 'freed: 0 B, 0 frames')",
  'diff_frames':         "hud('diff_frames · now 0.5s vs graded 0.5s: 77% changed, major (PSNR 10.5 dB)') && rail('diff_frames', 'changed: 77%, major')",
  'lint_frame':          "hud('lint_frame 1s · low-res upscale ×1') && rail('lint_frame', 'low-res upscale ×1')",
  # synthetic (see docstring)
  'set_ocio_view_delivery#2': "row('view', 'ACES 1.0 SDR') && !row('view', '✗ ACES 1.0 SDR') && !!document.querySelector('[data-panel=pipe] .row.emb') && row('output', 'Rec.709 sRGB') && !T('[data-panel=pipe] .row').some(s => s.includes('✗'))",
  'lint_frame#2':        "hud('lint_frame 2s · out of bounds ×1') && rail('lint_frame', 'out of bounds ×1')",
}
SYNTH = [
  {'tool_name': 'set_ocio_view_delivery', 'call_id': 'syn-1', 'phase': 'start', 'target': {}, 'payload': {'args': {'selection': {'config_path': '/cfg/studio.ocio', 'source': 'ACEScg', 'display': 'sRGB', 'view': 'ACES 1.0 SDR', 'input_gamut': 3, 'output_gamut': 0, 'output_transfer': 1}}}},
  {'tool_name': 'set_ocio_view_delivery', 'call_id': 'syn-1', 'phase': 'complete', 'target': {}, 'payload': {'dur_ms': 0.4, 'mutated': True, 'result': {'ok': True}}},
  {'tool_name': 'lint_frame', 'call_id': 'syn-2', 'phase': 'start', 'target': {}, 'payload': {'args': {'time': 2.0}}},
  {'tool_name': 'lint_frame', 'call_id': 'syn-2', 'phase': 'complete', 'target': {}, 'payload': {'dur_ms': 300, 'mutated': False, 'result': {
    'frames': [{'time': 2.0, 'findings': [{'code': 'out_of_bounds', 'rect': [1100.0, 40.0, 1400.0, 160.0], 'severity': 'warn', 'layer': {'id': 'clip1003', 'name': None}}]}],
    'summary': {'counts_by_code': {'out_of_bounds': 1}, 'frames_checked': 1}}}},
]
QA = {'diff_frames', 'lint_frame', 'cache_clear', 'cache_set_budget'}
RAW = r"/\b(?:c|clip|t|track|fx|effect|node|n|layer|l|asset|media)[_-]?\d+(?:[_-][a-z]{0,5}\d+|[_-][a-z])*\b/i"
PANELS = '[data-panel=agent] .call, [data-panel=pipe], [data-panel=hist], [data-panel=diff], [data-panel=media], [data-panel=insp], [data-panel=viewer], [data-panel=tl] .clip, [data-panel=tl] .trk .lab'

class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a): pass

async def main():
    from playwright.async_api import async_playwright
    fails = []
    srv = socketserver.ThreadingTCPServer(('127.0.0.1', 0), functools.partial(Quiet, directory=str(CRATE))); srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    async with async_playwright() as p:
        b = await p.chromium.launch()
        ctx = await b.new_context(viewport={'width': 1376, 'height': 768}); await ctx.add_init_script(PROBE)
        pg = await ctx.new_page(); errs = []
        pg.on('pageerror', lambda e: errs.append(str(e)))
        await pg.goto(f'http://127.0.0.1:{srv.server_address[1]}/ui/index.html?feed=off')
        await pg.wait_for_function('window.__kimo && __kimo.metrics.renders > 0')
        await pg.evaluate(JS)
        fam = await pg.evaluate("async names => { const r = await import('./router.js'); return names.map(n => [n, r.family(n)]); }", TOOLS)
        for n, f in fam:
            ok = f not in ('other',)
            print(f"{'PASS' if ok else 'FAIL'}  family  {n:<24} {f}")
            if not ok: fails.append('family:' + n)
        seen, start = {}, {}
        async def frame(): await pg.wait_for_timeout(50)
        for e in EV + SYNTH:
            name = e.get('tool_name')
            if name in TOOLS and e.get('phase') in ('complete', 'error'):
                k = seen[name] = seen.get(name, 0) + 1
                key = f'{name}#{k}' if f'{name}#{k}' in CHECK else (name if k == 1 and name in CHECK else None)
                if key is None: print(f'FAIL  no check for {name} #{k}'); fails.append(f'{name}#{k}'); continue
                await frame(); before = await pg.evaluate(CHECK[key]); m0 = await pg.evaluate('__kimo.state.mode'); v0 = await pg.evaluate("[...document.querySelectorAll('[data-panel=viewer] .frame > :not(.hud)')].map(e => e.outerHTML).join('')")
                await pg.evaluate('e => __kimo.apply(e)', e); await frame()
                after = await pg.evaluate(CHECK[key])
                bad = await pg.evaluate(f"""() => [...document.querySelectorAll('{PANELS}')].map(x => x.textContent).filter(s => {RAW}.test(s) || /\\b(undefined|NaN|null)\\b/.test(s)).map(s => s.slice(0, 80))""")
                mode = await pg.evaluate('__kimo.state.mode')
                still = True
                if name in QA:   # read / tune only: the workspace does not hop and the canvas is untouched
                    v1 = await pg.evaluate("[...document.querySelectorAll('[data-panel=viewer] .frame > :not(.hud)')].map(e => e.outerHTML).join('')")
                    still = mode == m0 and v1 == v0
                    if not still: print(f'      {key}: mode {m0} -> {mode} or the canvas changed')
                ok = (not before) and after and not bad and still
                print(f"{'PASS' if ok else 'FAIL'}  {key:<26} [{mode}] before={before!s:<5} after={after!s:<5}" + (f' raw/bad text: {bad[:2]}' if bad else ''), flush=True)
                if not ok: fails.append(key)
            else:
                await pg.evaluate('e => __kimo.apply(e)', e)
        missing = [t for t in TOOLS if t not in seen]
        if missing: print('FAIL  never exercised:', missing); fails += missing
        await pg.wait_for_timeout(2000)
        r0 = await pg.evaluate('__raf'); await pg.wait_for_timeout(3000); r1 = await pg.evaluate('__raf')
        print(f"{'PASS' if r1 == r0 else 'FAIL'}  idle after the replay: {r1 - r0} rAF in 3 s")
        if r1 != r0: fails.append('idle')
        if errs: print('page errors:', errs); fails.append('pageerror')
        await b.close()
    srv.shutdown()
    print(f"cov_c: {len(TOOLS) - len({f.split('#')[0].split(':')[-1] for f in fails if not f in ('idle', 'pageerror')})}/{len(TOOLS)} tools reactive · {'FAIL ' + ', '.join(fails) if fails else 'all green'}")
    sys.exit(1 if fails else 0)

if __name__ == '__main__':
    asyncio.run(main())
