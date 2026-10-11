"""fracture_glass realism harness (MCP only). usage: python3 tests/fracture_glass_realism_proof.py <tag> [key=value ...effect param overrides]
Needs /tmp/plates/style02.json (film agent's native project with plate paths repointed) and the two mixkit plates."""
import sys, os, time, json; sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'examples'))
from mcp_client import Kimo
from PIL import Image
tag = sys.argv[1]; ov = dict(a.split("=") for a in sys.argv[2:])
k = Kimo(); C = k.call; C("load_project", path=os.environ.get("PROJ", "/tmp/plates/style02.json"))
p = C("get_project"); fx = None
for t in p["tracks"]:
    for c in t["clips"]:
        for e in c["effects"]:
            if e["kind"] == "fracture_glass": fx = (c["id"], e["id"])
print("fx", fx)
for kx, v in ov.items(): C("set_effect_param", clip_id=fx[0], effect_id=fx[1], param=kx, value=float(v))
times = [float(x) for x in os.environ.get("TIMES", "8.34,8.42,8.52,8.66,8.84,9.1").split(",")]; ims = []
for i, x in enumerate(times):
    s = time.time(); pth = f"/tmp/fg_{tag}_{i}.png"; C("render_frame", time=x, path=pth); print("t", x, round(time.time() - s, 1), "s"); ims.append(Image.open(pth).convert("RGB"))
sheet = Image.new("RGB", (1920, 720)); [sheet.paste(im.resize((640, 360)), ((i % 3) * 640, (i // 3) * 360)) for i, im in enumerate(ims)]; sheet.save(f"/tmp/fg_{tag}_sheet.png")
