# HUD shot with impact landings AND beat-locked camera shake (hits measured from the reference: every 0.48 s = 125 BPM).
import sys; sys.path.insert(0,'/home/sandbox/kino/examples')
from mcp_client import Kino
src=open('/home/sandbox/kino/examples/scumble_hud_boxes.py').read().split("if len(sys.argv)")[0]
exec(src)
t=trk(); k.call("add_camera_shake",track_id=t,start=0,duration=D,hits=[s0+0.44 for (_,_,_,_,_,_,_,_,s0) in boxes])
if len(sys.argv)>1: k.call("export",path="/tmp/shake.mp4",crf=17)
