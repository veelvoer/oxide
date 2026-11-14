import sys, os, re, glob
from PIL import Image
import numpy as np
def load(dir, out=None):
    res=[]
    for f in sorted(glob.glob(dir+'/f*_t*_*.raw')):
        m=re.match(r'.*/f(\d+)_t(\d+)_(.+)_(\d+)x(\d+)\.raw',f)
        res.append((int(m[1]),int(m[2]),m[3],int(m[4]),int(m[5]),f))
    return res
def img(f,w,h):
    return Image.frombytes('RGBA',(w,h),open(f,'rb').read(),'raw','BGRA').convert('RGB')
def sheet(dir, crop, cols, out, step=1, scale=1.0, label=True):
    fr=load(dir)
    if fr and fr[0][3]<1100: crop=tuple(c//2 for c in crop); scale=scale*2
    fr=[x for x in fr if x[2]=='winit'][::step]
    if not fr: print('no frames'); return
    x0,y0,x1,y1=crop; tw,th=int((x1-x0)*scale),int((y1-y0)*scale); rows=(len(fr)+cols-1)//cols
    from PIL import ImageDraw
    sh=Image.new('RGB',(cols*tw,rows*(th+14)),(40,40,40)); d=ImageDraw.Draw(sh); t0=fr[0][1]
    for i,(n,t,name,w,h,f) in enumerate(fr):
        im=img(f,w,h).crop(crop).resize((tw,th)); r,c=divmod(i,cols)
        sh.paste(im,(c*tw,r*(th+14)+14)); d.text((c*tw+2,r*(th+14)),f'#{n} +{t-t0}ms',fill=(255,255,0))
    sh.save(out); print(out, sh.size, len(fr),'frames')
def analyze(dir, crop=None, output='winit', thr=0.0008):
    fr=[x for x in load(dir) if x[2]==output]
    if len(fr)<3: print(dir,'too few frames'); return
    ims=[]
    for n,t,name,w,h,f in fr:
        a=np.asarray(img(f,w,h)).astype(np.float32)/255
        if crop:
            x0,y0,x1,y1=[c//2 if w<1100 else c for c in crop]; a=a[y0:y1,x0:x1]
        ims.append(a)
    dts=[fr[i+1][1]-fr[i][1] for i in range(len(fr)-1)]
    d=[float(np.abs(ims[i+1]-ims[i]).mean()) for i in range(len(ims)-1)]
    act=[i for i,x in enumerate(d) if x>thr]
    slow=[i+1 for i,x in enumerate(dts) if x>25]
    print(f'{os.path.basename(dir)}: {len(fr)} frames, dt max {max(dts)}ms, slow frames(>25ms): {slow}')
    if not act: print('   no motion'); return
    a0,a1=act[0],act[-1]
    flick=[i+1 for i in range(a0,min(a1,len(ims)-2)) if np.abs(ims[i+2]-ims[i]).mean()<thr*0.3 and d[i]>thr*3 and d[i+1]>thr*3]
    stalls=[i+1 for i in range(a0,a1+1) if d[i]<=thr]
    print(f'   active frames [{a0+1}] .. [{a1+1}]  flicker: {flick}  stalls inside motion: {stalls}')
