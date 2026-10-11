"""End-to-end proof for the track-diagnostics tools on a synthetic plate with KNOWN corners (ground truth is used here only to
grade the tools; the tools themselves never see it). Usage: python3 tests/track_diag_proof.py <dir containing plate.mp4 and gt.json>"""
import json, subprocess, select, sys, numpy as np
d = sys.argv[1] if len(sys.argv) > 1 else "."
p = subprocess.Popen(["target/release/kino-mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True); i = 0
def rpc(m, params):
    global i; i += 1; p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": m, "params": params}) + "\n"); p.stdin.flush()
    if not select.select([p.stdout], [], [], 110)[0]: raise TimeoutError
    return json.loads(p.stdout.readline())
def call(n, **a):
    r = rpc("tools/call", {"name": n, "arguments": a})["result"]; t = r["content"][0]["text"]
    try: return json.loads(t), r.get("isError")
    except ValueError: return t, r.get("isError")
rpc("initialize", {}); call("project_set", width=640, height=360, fps=24)
a = call("import_media", path=f"{d}/plate.mp4")[0]; t = call("add_track")[0]["track_id"]
cid = call("add_clip", track_id=t, source_type="media", asset_id=a["asset_id"], start=0, duration=a["duration"])[0]["clip_id"]
gt = np.array(json.load(open(f"{d}/gt.json")))
ts = call("track_features", clip_id=cid, roi=[150/640, 80/360, 490/640, 80/360, 490/640, 320/360, 150/640, 320/360])[0]
fit = call("fit_track_homography", corners=(gt[0] / [640, 360]).ravel().tolist(), include_rows=False)[0]
def err(fid):
    fi = call("fit_info", fit_id=fid, frames=list(range(len(gt))))[0]
    return max(np.linalg.norm(np.array(x["corners_px"]).reshape(4, 2) - gt[x["rel_frame"]], axis=1).max() for x in fi["frames_data"])
e1 = err(fit["fit_id"]); ecc = call("ecc_refine_track", fit_id=fit["fit_id"], include_rows=False)[0]; e2 = err(ecc["fit_id"])
print("fit max corner err px", e1, "ecc", e2); assert e1 < 1.0 and e2 < 0.5
for bad in [dict(clip_id=cid, roi=[0, 0, 1, 0, 1, 1]), dict(clip_id=cid, roi=[.1]*8), dict(clip_id=cid, roi=[.2,.2,.7,.2,.7,.8,.2,.8], bogus=1), dict(clip_id=cid, roi=[.2,.2,.7,.2,.7,.8,.2,.8], max_features=2.5)]:
    r, e = call("track_features", **bad); assert e, bad
print("ok")
