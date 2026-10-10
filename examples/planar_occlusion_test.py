# Ground-truth test for track_planar under occlusion: a textured plate drifts, a skin-coloured "finger" slides in from the top and covers ~55% of it.
# Reports mean/max corner error (px) overall and during occlusion. usage: planar_occlusion_test.py <texture> [expand-less]
import sys,time,subprocess,os,numpy as np;sys.path.insert(0,'/home/sandbox/kino/examples');from mcp_client import Kino
from PIL import Image,ImageDraw,ImageFilter
TEX=sys.argv[1];tex=Image.open(TEX).convert('RGB').resize((640,440));W,H,N=1280,720,60
def quad(i):
    t=i/(N-1);return np.array([[253+80*t,299-20*t],[749+60*t,140+10*t],[1061+40*t,458+0*t],[521+80*t,620-10*t]],float)
def coef(q):
    src=np.array([[0,0],[640,0],[640,440],[0,440]],float);A=[];B=[]
    for (x,y),(X,Y) in zip(q,src): A+=[[x,y,1,0,0,0,-X*x,-X*y],[0,0,0,x,y,1,-Y*x,-Y*y]];B+=[X,Y]
    return tuple(np.linalg.solve(np.array(A),np.array(B)))
os.makedirs('/tmp/pof',exist_ok=True)
for i in range(N):
    c=coef(quad(i));plate=tex.transform((W,H),Image.PERSPECTIVE,c,Image.BICUBIC);m=Image.new('L',(640,440),255).transform((W,H),Image.PERSPECTIVE,c)
    bg=Image.new('RGB',(W,H),(60,52,50));bg.paste(plate,(0,0),m)
    d=np.clip((i-12)/16,0,1)*(1-np.clip((i-48)/10,0,1))  # finger slides in 12..28, out 48..58
    if d>0:
        f=Image.new('RGBA',(W,H),(0,0,0,0));dr=ImageDraw.Draw(f);y=-200+d*480;dr.ellipse([480,y-420,880,y+40],fill=(225,160,130,255));f=f.filter(ImageFilter.GaussianBlur(3));bg.paste(f,(0,0),f)
    bg.save('/tmp/pof/f%03d.png'%i)
subprocess.run(['ffmpeg','-v','error','-y','-framerate','30','-i','/tmp/pof/f%03d.png','-c:v','libx264','-crf','10','-pix_fmt','yuv420p','/tmp/po.mp4'],check=True)
k=Kino();C=k.call;C('project_set',width=W,height=H,fps=30)
a=C('import_media',path='/tmp/po.mp4')['asset_id'];t0=C('add_track')['track_id'];cl=C('add_clip',track_id=t0,source_type='media',asset_id=a,start=0,duration=N/30)['clip_id']
q0=quad(0);r=C('track_planar',clip_id=cl,corners=[v for p in q0 for v in (p[0]/W,p[1]/H)],start=0,end=N-1,return_quads=True)
Q=np.array(r['quads']).reshape(len(r['quads']),4,2)
err=np.array([np.abs(Q[i]-quad(i)).max() if Q.max()>2 else np.abs(Q[i]*[W,H]-quad(i)).max() for i in range(N)])
occ=slice(22,46)
print('lost',r['lost_frames'],'mean_conf %.3f'%r['mean_conf'],'| max corner err px: overall %.1f mean %.1f | during occlusion max %.1f mean %.1f | final %.1f'%(err.max(),err.mean(),err[occ].max(),err[occ].mean(),err[-1]))
