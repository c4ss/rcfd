import math
def order(f1,f2,f3,r21,r32):
    e21,e32=f2-f1,f3-f2
    s=math.copysign(1,e32/e21); p=1.0
    for _ in range(500):
        q=math.log((r21**p-s)/(r32**p-s))
        pn=abs(math.log(abs(e32/e21))+q)/math.log(r21)
        if abs(pn-p)<1e-12: return s,pn
        p=pn
    return s,float('nan')
cells=(38184,13627,4920)
r21=(cells[0]/cells[1])**(1/3); r32=(cells[1]/cells[2])**(1/3)
w=1486*2*math.pi/60
print('r21',r21,'r32',r32,'omega',w,'T_meas',333.30/w)
for name,f in (('torque',(2.098164,2.028692,1.918189)),('dp',(73.988,64.2329,46.778))):
    s,p=order(*f,r21,r32)
    e21=f[1]-f[0]
    fext=f[0]+(f[0]-f[1])/(r21**p-1)
    pb=min(p,1.0)
    band=1.25*abs(e21)/(r21**pb-1)
    print(name,'p',p,'fext',fext,'band',band,'rel',band/abs(f[0]))
    if name=='torque': print(' power f1',f[0]*w,'band W',band*w,'rel to meas',band*w/333.30, 'diff',f[0]*w-333.30)
    else: print(' diff',f[0]-126.46)
