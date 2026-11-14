import subprocess, json, time, os
def env():
    e=dict(os.environ); e.update(OXIDE_SOCKET='/tmp/ox/oxide.sock'); return e
def ctl(*args):
    return subprocess.run([os.path.expanduser('~/oxide-target/release/oxide-ctl'),*map(str,args)],capture_output=True,text=True,env=env())
def shot(path,output=None): return ctl('screenshot',path) if False else _shot(path,output)
def _shot(path,output):
    a=['screenshot'] 
    r=subprocess.run(['bash','-c','source /tmp/ox/lib.sh; WAYLAND_DISPLAY=$OX grim -o %s %s'%(output or 'winit',path)],capture_output=True,text=True)
    return r
def inp(*args): return ctl('input',*args)
def move(x,y): inp('move',x,y)
def click(x,y,btn='left'): inp('click',x,y,btn)
def key(name,*mods): inp('key',name,*mods)
def typ(text): inp('type',text)
def sleep(s): time.sleep(s)
def state(): return json.loads(ctl('state').stdout)['state']
def record(name,frames):
    d='/tmp/ox/rec_'+name
    subprocess.run(['rm','-rf',d]); ctl('record',d,str(frames)); time.sleep(0.25); return d
