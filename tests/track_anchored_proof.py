"""Proof for track_planar_anchored on a synthetic plate with KNOWN corners (truth grades the tool only; the tool sees just the anchors).
Usage (run from the repo root, plate dir outside the repo): python3 tests/track_anchored_proof.py <dir with plate.mp4 and gt.json>
Asserts: result passes through anchors; error with 2 anchors is far below the unanchored track; more anchors never worse by max."""
import sys, json, os
sys.path.insert(0, 'examples'); from mcp_client import Kimo
D = sys.argv[1]; k = Kimo(); C = k.call; C('project_set', width=640, height=400, fps=25)
gt = json.load(open(os.path.join(D, 'gt.json'))); N = len(gt)
a = C('import_media', path=os.path.join(D, 'plate.mp4'))['asset_id']; t = C('add_track')['track_id']
c = C('add_clip', track_id=t, source_type='media', asset_id=a, start=0, duration=N / 25, fit='cover')['clip_id']
nq = lambda q: [v / (640 if i % 2 == 0 else 400) for i, v in enumerate([x for pt in q for x in pt])]
def err(qs): return [max(((qs[i][2*j]-gt[i][j][0])**2+(qs[i][2*j+1]-gt[i][j][1])**2)**.5 for j in range(4)) for i in range(N)]
b = C('track_planar', clip_id=c, corners=nq(gt[0]), return_quads=True); eb = err(b['quads'])
print('unanchored max %.2f mean %.2f' % (max(eb), sum(eb) / N))
res = {}
for idx in ([0, N-1], [0, N//2, N-1], [0, N//3, 2*N//3, N-1]):
    r = C('track_planar_anchored', clip_id=c, anchors=[{'frame': i, 'corners': nq(gt[i])} for i in idx], return_quads=True); e = err(r['quads'])
    assert all(e[i] < 1e-6 for i in idx), 'does not pass through anchors'
    res[len(idx)] = max(e); print(idx, 'max %.2f mean %.2f' % (max(e), sum(e) / N), 'flagged gaps', r['gaps_needing_anchor'])
assert res[2] < 0.25 * max(eb), 'anchoring did not cut max error 4x'
assert res[4] <= res[2] + 1e-6
try: C('track_planar_anchored', clip_id=c, anchors=[{'frame': 0, 'corners': nq(gt[0])}]); raise SystemExit('single anchor should fail')
except Exception: pass
print('PASS')
