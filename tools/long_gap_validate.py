#!/usr/bin/env python3
"""Reproducible real-footage evidence; no dependencies besides Python, cargo, ffmpeg.
Run from repository root: python3 tools/long_gap_validate.py /tmp/long-gap-proof
Downloads OpenCV testdata faceocc2, not owned footage. Reports failures, never
uses benchmark annotations to adjust tracking. Artificial blackout is explicitly
labelled, and natural footage is scored separately.
"""
import collections
import json
import math
from pathlib import Path
import subprocess
import sys
import urllib.request

out = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/long-gap-proof')
out.mkdir(parents=True, exist_ok=True)
base = 'https://raw.githubusercontent.com/opencv/opencv_extra/5.x/testdata/cv/tracking/faceocc2/'
for name, tail in [('faceocc2.webm', 'data/faceocc2.webm'), ('gt.txt', 'gt.txt')]:
    if not (out / name).exists():
        urllib.request.urlretrieve(base + tail, out / name)
subprocess.run(['ffmpeg', '-y', '-v', 'error', '-i', str(out/'faceocc2.webm'), '-pix_fmt', 'gray', '-f', 'rawvideo', str(out/'faceocc2.raw')], check=True)
data = (out/'faceocc2.raw').read_bytes()
w, h, size = 320, 240, 320*240
frames = [data[i:i+size] for i in range(0, len(data), size)]
post = []
for f in frames[40:101]:
    g = bytearray(size)
    for y in range(h):
        g[y*w+60:(y+1)*w] = f[y*w:y*w+w-60]
    post.append(g)
(out/'controlled.raw').write_bytes(b''.join(frames[:8])+bytes(size*300)+b''.join(post))
subprocess.run(['ffmpeg','-y','-v','error','-f','rawvideo','-pix_fmt','gray','-s','320x240','-r','25','-i',str(out/'controlled.raw'),'-c:v','libx264','-crf','0','-pix_fmt','yuv420p',str(out/'controlled.mp4')],check=True)
subprocess.run(['cargo','build','--release','-p','kino-core','--example','long_gap_proof','--example','long_gap_clip'],check=True)
gt = [list(map(float,line.replace(',', ' ').split())) for line in (out/'gt.txt').read_text().splitlines()]
results = {}
for name, cmd in [('natural',['target/release/examples/long_gap_proof',str(out/'faceocc2.raw'),'320','240','118','57','82','98']),('controlled_command',['target/release/examples/long_gap_clip',str(out/'controlled.mp4')])]:
    result = json.loads(subprocess.check_output(cmd))
    (out/(name+'.json')).write_text(json.dumps(result,indent=2))
    samples = result['samples']
    errors = []
    for s in samples:
        i = s['frame']
        if not s['bbox'] or (name != 'natural' and i < 308):
            continue
        a = gt[i if name == 'natural' else i-308+40]
        b = s['bbox']
        errors.append(math.hypot(b['x']+b['w']/2-a[0]-a[2]/2-(0 if name=='natural' else 60),b['y']+b['h']/2-a[1]-a[3]/2))
    results[name] = dict(seconds=result['seconds'],frames=len(samples),states=dict(collections.Counter(s['state'] for s in samples)),reacquisitions=[s['frame'] for s in samples if s['state']=='reacquired'],reported_scored_boxes=len(errors),mean_center_error_px=sum(errors)/len(errors) if errors else None,max_center_error_px=max(errors) if errors else None)
    if name != 'natural':
        assert all(s['bbox'] is None for s in samples[8:308])
        assert samples[308]['state']=='tentative' and samples[308]['bbox'] is None
        assert samples[309]['state']=='reacquired'
        results[name]['artificial_gap_frames']=300
print(json.dumps(results,indent=2))
(out/'metrics.json').write_text(json.dumps(results,indent=2))
