import sys, math
sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
import skins_v2 as sk   # glyph() from the pre-luminous version has the tile-style glyphs

def f(v):
    s = f"{v:.1f}"; return s[:-2] if s.endswith('.0') else s

# ---------- new sample ship (cells of 20, 12 x 7, nose up) ----------
SL = [  # slopes
 [(60,20),(100,0),(100,20)], [(140,0),(180,20),(140,20)],
 [(20,40),(60,20),(60,40)], [(180,20),(220,40),(180,40)],
 [(0,60),(20,40),(20,60)], [(220,40),(240,60),(220,60)],
 [(0,80),(20,80),(20,100)], [(220,80),(240,80),(220,100)],
 [(20,100),(40,100),(40,120)], [(200,100),(220,100),(200,120)],
 [(60,120),(80,120),(80,140)], [(160,120),(180,120),(160,140)],
]
T = []  # (kind, x, y, w, h)
T += [('bem',100,0,20,40), ('bem',120,0,20,40)]
T += [('blk',60,20,20,20), ('cap',80,20,20,20), ('cap',140,20,20,20), ('blk',160,20,20,20)]
T += [('blk',20,40,20,20), ('rad',40,40,20,20), ('fis',60,40,20,20), ('shd',80,40,20,20), ('fus',100,40,40,40),
      ('shd',140,40,20,20), ('fis',160,40,20,20), ('rad',180,40,20,20), ('blk',200,40,20,20)]
T += [('pls',0,60,20,20), ('cap',20,60,20,20), ('blk',40,60,20,20), ('blk',60,60,20,20), ('cor',80,60,20,20),
      ('cor',140,60,20,20), ('blk',160,60,20,20), ('blk',180,60,20,20), ('cap',200,60,20,20), ('pls',220,60,20,20)]
T += [('rad',20,80,20,20), ('blk',40,80,20,20), ('cgo',60,80,20,20), ('blk',80,80,20,20), ('blk',100,80,20,20),
      ('blk',120,80,20,20), ('blk',140,80,20,20), ('cgo',160,80,20,20), ('blk',180,80,20,20), ('rad',200,80,20,20)]
T += [('thr',40,100,20,20), ('blk',60,100,20,20), ('rad',80,100,20,20), ('tor',100,100,40,40), ('rad',140,100,20,20),
      ('blk',160,100,20,20), ('thr',180,100,20,20)]
T += [('blk',80,120,20,20), ('blk',140,120,20,20)]

def slope(poly):
    pts = " ".join(f"{x},{y}" for x, y in poly)
    edges = [(poly[i], poly[(i + 1) % 3]) for i in range(3)]
    (x1, y1), (x2, y2) = max(edges, key=lambda e: math.dist(*e))
    cx = sum(x for x, _ in poly) / 3; cy = sum(y for _, y in poly) / 3
    def ins(x, y):
        dx, dy = cx - x, cy - y; L = math.hypot(dx, dy)
        return x + dx / L * 2.4, y + dy / L * 1.6
    a, b = ins(x1, y1), ins(x2, y2)
    return (f'<polygon points="{pts}" fill="url(#gA)" stroke="#0a0f16" stroke-width=".8"></polygon>'
            f'<path d="M{f(a[0])} {f(a[1])}L{f(b[0])} {f(b[1])}" stroke="#d8f5ee" stroke-opacity=".22" stroke-width="1.1"></path>')

body = "".join(slope(p) for p in SL) + "".join(
    f'<use href="#s-{k}" x="{x}" y="{y}" width="{w}" height="{h}"></use>' for k, x, y, w, h in T)
# all tile outlines (for the neon seam glow)
outl = "".join(f"M{x} {y}h{w}v{h}h-{w}z" for k, x, y, w, h in T) + "".join(
    "M" + " ".join(f"{x} {y}" for x, y in p) + "z" for p in SL)

# ---------- raider (3x3, centred, nose up) ----------
RSL = [[(-30,-10),(-10,-30),(-10,-10)], [(10,-30),(30,-10),(10,-10)], [(-30,10),(-10,10),(-10,30)], [(10,10),(30,10),(10,30)]]
RT = [('cnn',-10,-30,20,20), ('blk',-30,-10,20,20), ('acore',-10,-10,20,20), ('blk',10,-10,20,20), ('thr',-10,10,20,20)]
rbody = "".join(slope(p) for p in RSL).replace('url(#gA)', 'url(#gR)') + "".join(
    f'<use href="#s-{k}{"r" if k == "blk" else ""}" x="{x}" y="{y}" width="{w}" height="{h}"></use>' for k, x, y, w, h in RT)
routl = "".join(f"M{x} {y}h{w}v{h}h-{w}z" for k, x, y, w, h in RT) + "".join(
    "M" + " ".join(f"{x} {y}" for x, y in p) + "z" for p in RSL)

# ---------- shared tile symbols ----------
def bevel(w, h, a=.2, s=.32):
    return (f'<path d="M1.6 {f(h-1.6)}V1.6H{f(w-1.6)}" fill="none" stroke="#d8f5ee" stroke-opacity="{a}" stroke-width="1.1"></path>'
            f'<path d="M{f(w-1)} 1.8V{f(h-1)}H1.8" fill="none" stroke="#000" stroke-opacity="{s}"></path>')
def housing(w, h):
    return (f'<rect x=".5" y=".5" width="{w-1}" height="{h-1}" fill="url(#gH)" stroke="#0a0f16" stroke-width=".8"></rect>{bevel(w,h,.15,.38)}'
            f'<rect x="2.6" y="2.6" width="{f(w-5.2)}" height="{f(h-5.2)}" rx="1.6" fill="url(#gBay)" stroke="#070b10" stroke-width=".6"></rect>')
def glyph(k, w, h):
    if k == 'cnn':
        g = '<circle cx="10" cy="11" r="4" fill="#ec8f7c"></circle>'
        c = '<circle cx="10" cy="11" r="4.4" fill="url(#gRed)"></circle><circle cx="10" cy="11" r="3.4" fill="none" stroke="#ec8f7c" stroke-width=".9"></circle><path d="M10 7.6V2" stroke="#ec8f7c" stroke-opacity=".8" stroke-width="1.3"></path>'
        return g, c
    if k == 'acore':
        g = '<circle cx="10" cy="10" r="4" fill="#ec8f7c"></circle>'
        c = '<circle cx="10" cy="10" r="4.4" fill="url(#gRed)"></circle><circle cx="10" cy="10" r="3.6" fill="none" stroke="#ec8f7c" stroke-width=".9"></circle><circle cx="10" cy="10" r="1.3" fill="#ffe8e2"></circle>'
        return g, c
    g, c = sk.glyph('', k, w, h)
    rep = {'url(#gCore)': 'url(#gCore)', '#79d3a4': '#8cefdb', '#c9f6df': '#e0fff7', '#86a6ee': '#8fb0f2', '#e47b69': '#ec8f7c', '#f0a090': '#f4a998'}
    for a, b in rep.items():
        g = g.replace(a, b); c = c.replace(a, b)
    return g, c
syms = [f'<symbol id="s-blk" viewBox="0 0 20 20"><rect x=".5" y=".5" width="19" height="19" fill="url(#gA)" stroke="#0a0f16" stroke-width=".8"></rect>{bevel(20,20)}</symbol>',
        f'<symbol id="s-blkr" viewBox="0 0 20 20"><rect x=".5" y=".5" width="19" height="19" fill="url(#gR)" stroke="#0a0f16" stroke-width=".8"></rect>{bevel(20,20)}</symbol>']
for k, (w, h) in {'fis': (20,20), 'cap': (20,20), 'shd': (20,20), 'rad': (20,20), 'cor': (20,20), 'cgo': (20,20), 'thr': (20,20),
                  'pls': (20,20), 'bem': (20,40), 'fus': (40,40), 'tor': (40,40), 'acore': (20,20), 'cnn': (20,20)}.items():
    g, c = glyph(k, w, h)
    syms.append(f'<symbol id="s-{k}" viewBox="0 0 {w} {h}">{housing(w,h)}<g filter="url(#gl)" opacity=".75">{g}</g>{c}</symbol>')

open('syms.txt', 'w').write("\n".join(syms))
open('body.txt', 'w').write(body)
open('outl.txt', 'w').write(outl)
open('rbody.txt', 'w').write(rbody)
open('routl.txt', 'w').write(routl)
for n in ['syms', 'body', 'outl', 'rbody', 'routl']:
    print(n, len(open(n + '.txt').read()))
