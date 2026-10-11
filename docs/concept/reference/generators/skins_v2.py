"""Ship renderers for the UI mockups: drydock edit view (tiles) and flight skins A/B/C."""
from shapely.geometry import Polygon, box
from shapely.ops import unary_union

def f(v):
    s = f"{v:.1f}"
    return s[:-2] if s.endswith(".0") else s

def P(kind, x, y, w=20, h=20, poly=None):
    return dict(kind=kind, x=x, y=y, w=w, h=h, poly=poly)

MID = [
    P('slope', 0, 0, poly=[(140, 20), (140, 40), (80, 40)]),
    P('slope', 0, 0, poly=[(180, 20), (180, 40), (240, 40)]),
    P('slope', 0, 0, poly=[(80, 40), (80, 60), (40, 60)]),
    P('slope', 0, 0, poly=[(240, 40), (240, 60), (280, 60)]),
    P('slope', 0, 0, poly=[(80, 100), (100, 100), (100, 120)]),
    P('slope', 0, 0, poly=[(240, 100), (220, 100), (220, 120)]),
] + [P('blk', x, y) for x, y in [(80, 40), (100, 40), (200, 40), (220, 40), (120, 60), (180, 60),
                                  (80, 80), (120, 80), (180, 80), (220, 80)]] + [
    P('bem', 140, 20, 20, 40), P('bem', 160, 20, 20, 40),
    P('pls', 20, 40), P('pls', 280, 40),
    P('cap', 120, 40), P('cap', 180, 40), P('cap', 20, 60), P('cap', 280, 60),
    P('rad', 40, 60), P('rad', 260, 60), P('rad', 60, 80), P('rad', 240, 80), P('rad', 120, 100), P('rad', 180, 100),
    P('fis', 60, 60), P('fis', 240, 60),
    P('shd', 80, 60), P('shd', 220, 60),
    P('cor', 100, 60), P('cor', 200, 60),
    P('fus', 140, 60, 40, 40),
    P('cgo', 100, 80), P('cgo', 200, 80),
    P('thr', 100, 100), P('thr', 200, 100),
    P('tor', 140, 100, 40, 40),
]
MID_BARRELS = [(147.5, 6, 5, 20), (167.5, 6, 5, 20), (28, 28, 4, 14), (288, 28, 4, 14)]
MID_MUZZLES = [(150, 6), (170, 6), (30, 28), (290, 28)]
MID_PLUMES = [[(104, 119), (116, 119), (110, 142)], [(204, 119), (216, 119), (210, 142)],
              [(145, 139), (175, 139), (160, 180)]]
MID_SEAMS_B = ["M40 40H280", "M20 80H300", "M80 120H240", "M110 20V40M210 20V40",
               "M60 40V80M100 40V80M220 40V80M260 40V80", "M80 80V120M240 80V120"]
MID_SEAMS_C = ["M20 80H300", "M100 40V80M220 40V80"]
MID_STRIPES = ["M93 39 138 24", "M101 39.5 138 27.2", "M227 39 182 24", "M219 39.5 182 27.2"]
MID_LIGHTS = [(24, 76, '#e47b69'), (296, 76, '#79d3a4')]

# small automaton raider (nose up = -y), steel
AUTO = [
    P('slope', 0, 0, poly=[(-10, -10), (-10, 10), (-30, 10)]),
    P('slope', 0, 0, poly=[(10, -10), (10, 10), (30, 10)]),
    P('blk', -30, 10), P('blk', 10, 10),
    P('acore', -10, -10),
    P('thr', -10, 10),
]
AUTO_BARRELS = [(-2, -24, 4, 16)]
AUTO_MUZZLES = [(0, -24)]
AUTO_PLUMES = [[(-6, 29), (6, 29), (0, 48)]]
AUTO_LIGHTS = []


def fglyph(p, k, w, h):
    """Flight-skin detail: internals stay under the plating, only surface parts show."""
    if k == 'fis':
        return (f'<circle cx="10" cy="10" r="4.2" fill="#0b1119" stroke="#05080c" stroke-width=".6"></circle>'
                f'<circle cx="10" cy="10" r="3.4" fill="#e2b15c" opacity=".7" filter="url(#{p}gl)"></circle><circle cx="10" cy="10" r="2.6" fill="url(#{p}gCore)"></circle>')
    if k == 'fus':
        return (f'<circle cx="20" cy="20" r="14.5" fill="url(#{p}gBay)" stroke="#05080c" stroke-width=".8"></circle>'
                f'<circle cx="20" cy="20" r="14.5" fill="none" stroke="#eef5fa" stroke-opacity=".08" stroke-width=".8" transform="translate(.7 .7)"></circle>'
                f'<circle cx="20" cy="20" r="9" fill="none" stroke="#e2b15c" stroke-width="2.2" opacity=".75" filter="url(#{p}gl)"></circle>'
                f'<circle cx="20" cy="20" r="9" fill="none" stroke="#f2c472" stroke-width=".9"></circle>'
                f'<circle cx="20" cy="20" r="5" fill="url(#{p}gCore)"></circle>'
                f'<path d="M20 5.5v3M20 31.5v3M5.5 20h3M31.5 20h3" stroke="#e2b15c" stroke-opacity=".6" stroke-width="1.2"></path>')
    if k == 'shd':
        return (f'<circle cx="10" cy="11" r="4" fill="#86a6ee" opacity=".55" filter="url(#{p}gl)"></circle>'
                f'<circle cx="10" cy="11" r="3" fill="#121a2a" stroke="#86a6ee" stroke-width=".8"></circle><circle cx="10" cy="11" r="1.3" fill="#dfe8ff"></circle>')
    if k == 'rad':
        return (f'<path d="M6.5 6v8M10 6v8M13.5 6v8" stroke="#e47b69" stroke-width="1.6" opacity=".5" filter="url(#{p}gl)"></path>'
                f'<path d="M6.5 6v8M10 6v8M13.5 6v8" stroke="#c96a5a" stroke-opacity=".8" stroke-width=".8" stroke-linecap="round"></path>')
    if k == 'cgo':
        return '<rect x="5" y="5" width="10" height="10" rx="1.5" fill="none" stroke="#05080c" stroke-opacity=".55" stroke-width=".7"></rect><path d="M8 10h4" stroke="#8d9db4" stroke-opacity=".35"></path>'
    if k == 'pls':
        return (f'<circle cx="10" cy="11" r="5.4" fill="url(#{p}gBay)" stroke="#05080c" stroke-width=".7"></circle>'
                f'<circle cx="10" cy="11" r="2.8" fill="#5cc6d6" opacity=".7" filter="url(#{p}gl)"></circle><circle cx="10" cy="11" r="2.2" fill="url(#{p}gCy)"></circle>')
    if k == 'bem':
        return (f'<rect x="6.8" y="1" width="6.4" height="35" rx="3.2" fill="url(#{p}gBay)" stroke="#05080c" stroke-width=".7"></rect>'
                f'<path d="M10 3.5V33" stroke="#5cc6d6" stroke-width="2.2" opacity=".6" filter="url(#{p}gl)"></path>'
                f'<path d="M10 3.5V33" stroke="#bff3fb" stroke-width=".8"></path><circle cx="10" cy="31" r="2.4" fill="url(#{p}gCy)"></circle>')
    if k in ('thr', 'tor', 'acore'):
        return glyph_svg(p, k, w, h)
    return ''

PLATES_B = [(0,0,110,40,.035),(110,0,100,40,-.05),(210,0,110,40,.035),
            (0,40,60,40,-.04),(60,40,40,40,.05),(100,40,120,40,.0),(220,40,40,40,.05),(260,40,60,40,-.04),
            (0,80,80,40,.02),(80,80,160,40,-.06),(240,80,80,40,.02),(0,120,320,60,-.03)]
PLATES_C = [(0,0,100,80,.03),(100,0,120,80,-.04),(220,0,100,80,.03),(0,80,320,100,-.04)]
GREEBLES = ('<g fill="none" stroke="#05080c" stroke-opacity=".6" stroke-width=".7">'
            '<rect x="84" y="45" width="10" height="5" rx="1.2"></rect><rect x="226" y="45" width="10" height="5" rx="1.2"></rect></g>'
            '<path d="M84 88h10M84 91h10M84 94h10M226 88h10M226 91h10M226 94h10" stroke="#05080c" stroke-opacity=".55" stroke-width=".8"></path>')

MODULES = {'bem', 'pls', 'cap', 'rad', 'fis', 'shd', 'cor', 'fus', 'cgo', 'thr', 'tor', 'acore'}

def footprint(p):
    if p['poly']:
        return Polygon(p['poly'])
    return box(p['x'], p['y'], p['x'] + p['w'], p['y'] + p['h'])

def path_of(g):
    polys = [g] if g.geom_type == 'Polygon' else list(g.geoms)
    out = []
    for pg in polys:
        for ring in [pg.exterior] + list(pg.interiors):
            pts = list(ring.coords)[:-1]
            out.append("M" + " ".join(f"{f(x)} {f(y)}" for x, y in pts) + "Z")
    return "".join(out)

def hull_geom(parts, level):
    h = unary_union([footprint(p) for p in parts])
    if level == 'A':
        return h.buffer(-2.5, join_style='mitre').buffer(2.5, join_style='bevel')
    if level == 'B':
        h = h.buffer(-4, quad_segs=5).buffer(4, quad_segs=5)
        return h.buffer(3, quad_segs=5).buffer(-3, quad_segs=5)
    if level == 'C':
        h = h.buffer(12, quad_segs=6).buffer(-12, quad_segs=6)
        return h.buffer(-6, quad_segs=6).buffer(6, quad_segs=6)
    return h

MAT = {
    'composite': ('#47605f', '#2c3f40', '#192426'),
    'steel': ('#6c7c92', '#3b4759', '#1f2732'),
}

def defs(p, mat='composite', rot=0):
    a, b, c = MAT[mat]
    gt = f' gradientTransform="rotate({-rot} .5 .5)"' if rot else ''
    return f'''<linearGradient id="{p}gA" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{a}"></stop><stop offset=".55" stop-color="{b}"></stop><stop offset="1" stop-color="{c}"></stop></linearGradient>
<linearGradient id="{p}gHull" x1="0" y1="0" x2="1" y2="1"{gt}><stop offset="0" stop-color="{a}"></stop><stop offset=".5" stop-color="{b}"></stop><stop offset="1" stop-color="{c}"></stop></linearGradient>
<linearGradient id="{p}gRim" x1="0" y1="0" x2=".75" y2="1"{gt}><stop offset="0" stop-color="#eef6fa" stop-opacity=".8"></stop><stop offset=".45" stop-color="#eef6fa" stop-opacity="0"></stop></linearGradient>
<linearGradient id="{p}gH" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#2d3949"></stop><stop offset=".6" stop-color="#1d2533"></stop><stop offset="1" stop-color="#161d28"></stop></linearGradient>
<linearGradient id="{p}gBay" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#0a1018"></stop><stop offset="1" stop-color="#172131"></stop></linearGradient>
<radialGradient id="{p}gCore"><stop offset="0" stop-color="#fff4d6"></stop><stop offset=".4" stop-color="#f2c472" stop-opacity=".9"></stop><stop offset="1" stop-color="#e2b15c" stop-opacity="0"></stop></radialGradient>
<radialGradient id="{p}gCy"><stop offset="0" stop-color="#eafcff"></stop><stop offset=".45" stop-color="#5cc6d6" stop-opacity=".9"></stop><stop offset="1" stop-color="#5cc6d6" stop-opacity="0"></stop></radialGradient>
<radialGradient id="{p}gRed"><stop offset="0" stop-color="#ffe2da"></stop><stop offset=".45" stop-color="#e47b69" stop-opacity=".9"></stop><stop offset="1" stop-color="#e47b69" stop-opacity="0"></stop></radialGradient>
<linearGradient id="{p}gAmb" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f6d595"></stop><stop offset="1" stop-color="#a87a35"></stop></linearGradient>
<linearGradient id="{p}gNoz" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1c2a2c"></stop><stop offset="1" stop-color="#3f7d63"></stop></linearGradient>
<linearGradient id="{p}gBar" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#3a4757"></stop><stop offset=".45" stop-color="#b3c3d4"></stop><stop offset=".7" stop-color="#5a697e"></stop><stop offset="1" stop-color="#2b3441"></stop></linearGradient>
<linearGradient id="{p}gPl" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#d4fbe6" stop-opacity=".95"></stop><stop offset=".3" stop-color="#79d3a4" stop-opacity=".7"></stop><stop offset="1" stop-color="#79d3a4" stop-opacity="0"></stop></linearGradient>
<filter id="{p}gl" x="-60%" y="-60%" width="220%" height="220%"><feGaussianBlur stdDeviation="1.6"></feGaussianBlur></filter>
<filter id="{p}gl2" x="-60%" y="-60%" width="220%" height="220%"><feGaussianBlur stdDeviation="3.2"></feGaussianBlur></filter>'''

def glyph(p, k, w, h):
    """Luminous machine glyph in local coords; returns (glow_part, crisp_part)."""
    cx, cy = w / 2, h / 2
    if k == 'fis':
        g = f'<circle cx="10" cy="10" r="5" fill="#e2b15c"></circle>'
        c = f'<circle cx="10" cy="10" r="5.2" fill="url(#{p}gCore)"></circle><circle cx="10" cy="10" r="4.6" fill="none" stroke="#e2b15c" stroke-width=".9"></circle><circle cx="10" cy="10" r="1.5" fill="#fff4d6"></circle>'
    elif k == 'fus':
        g = f'<circle cx="20" cy="20" r="12" fill="none" stroke="#e2b15c" stroke-width="2.4"></circle><circle cx="20" cy="20" r="6" fill="#e2b15c"></circle>'
        c = (f'<circle cx="20" cy="20" r="13" fill="url(#{p}gCore)" opacity=".55"></circle>'
             f'<circle cx="20" cy="20" r="12" fill="none" stroke="#e2b15c" stroke-width="1.5"></circle>'
             f'<circle cx="20" cy="20" r="7" fill="none" stroke="#f2c472" stroke-width="1"></circle>'
             f'<circle cx="20" cy="20" r="3.2" fill="#fff4d6"></circle>'
             f'<g fill="url(#{p}gAmb)" stroke="#e2b15c" stroke-width=".6"><rect x="18.5" y="4.6" width="3" height="4"></rect><rect x="18.5" y="31.4" width="3" height="4"></rect><rect x="4.6" y="18.5" width="4" height="3"></rect><rect x="31.4" y="18.5" width="4" height="3"></rect></g>')
    elif k == 'cap':
        g = '<g fill="#e2b15c"><rect x="5" y="6.5" width="2.6" height="7"></rect><rect x="8.7" y="6.5" width="2.6" height="7"></rect><rect x="12.4" y="6.5" width="2.6" height="7"></rect></g>'
        c = f'<g fill="url(#{p}gAmb)" stroke="#e2b15c" stroke-width=".5"><rect x="5" y="6.5" width="2.6" height="7" rx=".6"></rect><rect x="8.7" y="6.5" width="2.6" height="7" rx=".6"></rect><rect x="12.4" y="6.5" width="2.6" height="7" rx=".6"></rect></g>'
    elif k == 'rad':
        g = '<path d="M5.5 4.5v11M8.5 4.5v11M11.5 4.5v11M14.5 4.5v11" stroke="#e47b69" stroke-width="1.6"></path>'
        c = '<path d="M5.5 4.5v11M8.5 4.5v11M11.5 4.5v11M14.5 4.5v11" stroke="#f0a090" stroke-width="1.1" stroke-linecap="round"></path>'
    elif k == 'shd':
        g = '<path d="M5.5 13a4.5 4.5 0 0 1 9 0M3.8 13a6.2 6.2 0 0 1 12.4 0" fill="none" stroke="#86a6ee" stroke-width="1.4"></path>'
        c = '<path d="M5.5 13a4.5 4.5 0 0 1 9 0M3.8 13a6.2 6.2 0 0 1 12.4 0" fill="none" stroke="#a9c0f4" stroke-width=".9"></path><circle cx="10" cy="13.4" r="1.6" fill="#dfe8ff"></circle>'
    elif k == 'cor':
        g = '<rect x="7" y="7" width="6" height="6" fill="#ae8cec"></rect>'
        c = '<path d="M8.5 7V4.6M11.5 7V4.6M8.5 13v2.4M11.5 13v2.4M7 8.5H4.6M7 11.5H4.6M13 8.5h2.4M13 11.5h2.4" stroke="#ae8cec" stroke-width=".8"></path><rect x="7" y="7" width="6" height="6" fill="#ae8cec" fill-opacity=".35" stroke="#c9b2f5" stroke-width=".8"></rect><rect x="9" y="9" width="2" height="2" fill="#efe6ff"></rect>'
    elif k == 'acore':
        g = '<circle cx="10" cy="9" r="4" fill="#e47b69"></circle>'
        c = f'<circle cx="10" cy="9" r="4.4" fill="url(#{p}gRed)"></circle><circle cx="10" cy="9" r="3.6" fill="none" stroke="#e47b69" stroke-width=".9"></circle><path d="M5 15h10" stroke="#e47b69" stroke-opacity=".6"></path>'
    elif k == 'cgo':
        g = ''
        c = '<g fill="#8d9db4" fill-opacity=".14" stroke="#8d9db4" stroke-opacity=".75" stroke-width=".7"><rect x="4.5" y="4.5" width="5" height="5" rx=".6"></rect><rect x="10.5" y="4.5" width="5" height="5" rx=".6"></rect><rect x="4.5" y="10.5" width="5" height="5" rx=".6"></rect><rect x="10.5" y="10.5" width="5" height="5" rx=".6"></rect></g>'
    elif k == 'thr':
        g = '<ellipse cx="10" cy="16.5" rx="4.5" ry="1.6" fill="#79d3a4"></ellipse>'
        c = f'<rect x="7.5" y="3.5" width="5" height="3.5" rx=".8" fill="#223032" stroke="#79d3a4" stroke-width=".7"></rect><path d="M8 7h4l3 10H5z" fill="url(#{p}gNoz)" stroke="#79d3a4" stroke-width=".8"></path><ellipse cx="10" cy="16.6" rx="4.4" ry="1.1" fill="#c9f6df"></ellipse>'
    elif k == 'tor':
        g = '<ellipse cx="20" cy="36" rx="12" ry="2.6" fill="#79d3a4"></ellipse>'
        c = (f'<rect x="15" y="5" width="10" height="9" rx="1.2" fill="#223032" stroke="#79d3a4" stroke-width=".8"></rect>'
             f'<path d="M15 14h10l8 22H7z" fill="url(#{p}gNoz)" stroke="#79d3a4" stroke-width=".9"></path>'
             '<path d="M11.4 25h17.2M9.6 30h20.8" stroke="#79d3a4" stroke-opacity=".55" stroke-width=".8"></path>'
             '<ellipse cx="20" cy="36" rx="12.5" ry="1.8" fill="#c9f6df"></ellipse>')
    elif k == 'pls':
        g = '<circle cx="10" cy="11" r="4" fill="#5cc6d6"></circle>'
        c = f'<circle cx="10" cy="11" r="4.6" fill="url(#{p}gCy)"></circle><circle cx="10" cy="11" r="3.6" fill="none" stroke="#5cc6d6" stroke-width=".9"></circle><path d="M10 7.4V2" stroke="#5cc6d6" stroke-opacity=".7" stroke-width="1.2"></path>'
    elif k == 'bem':
        g = '<circle cx="10" cy="30" r="5" fill="#5cc6d6"></circle><rect x="8.5" y="2" width="3" height="22" fill="#5cc6d6" opacity=".5"></rect>'
        c = (f'<rect x="8.3" y="2" width="3.4" height="23" rx="1" fill="#5cc6d6" fill-opacity=".22" stroke="#5cc6d6" stroke-opacity=".7" stroke-width=".6"></rect>'
             f'<circle cx="10" cy="30" r="6" fill="url(#{p}gCy)"></circle><circle cx="10" cy="30" r="4.8" fill="none" stroke="#5cc6d6" stroke-width=".9"></circle><circle cx="10" cy="30" r="2" fill="none" stroke="#eafcff" stroke-width=".8"></circle>')
    else:
        g, c = '', ''
    return g, c

def glyph_svg(p, k, w, h, glow=True, glow_op=.75):
    g, c = glyph(p, k, w, h)
    out = ''
    if glow and g:
        out += f'<g filter="url(#{p}gl)" opacity="{glow_op}">{g}</g>'
    return out + c

def bevel(w, h, a=.22, s=.3):
    return (f'<path d="M1.6 {f(h-1.6)}V1.6H{f(w-1.6)}" fill="none" stroke="#eef5fa" stroke-opacity="{a}" stroke-width="1.2"></path>'
            f'<path d="M{f(w-1)} 1.8V{f(h-1)}H1.8" fill="none" stroke="#000" stroke-opacity="{s}"></path>')

def edit_symbols(p, mat='composite'):
    """<symbol>s for the drydock edit view and palette icons."""
    out = [defs(p, mat)]
    weave = '<path d="M4 16 16 4M4 10.5 10.5 4M9.5 16 16 9.5" stroke="#bfe0dc" stroke-opacity=".07"></path>'
    out.append(f'<symbol id="{p}blk" viewBox="0 0 20 20"><rect x=".5" y=".5" width="19" height="19" fill="url(#{p}gA)" stroke="#0a0f16"></rect>{weave}{bevel(20,20)}</symbol>')
    def housing(w, h):
        return (f'<rect x=".5" y=".5" width="{w-1}" height="{h-1}" fill="url(#{p}gH)" stroke="#0a0f16"></rect>{bevel(w,h,.18,.35)}'
                f'<rect x="2.6" y="2.6" width="{f(w-5.2)}" height="{f(h-5.2)}" rx="1.6" fill="url(#{p}gBay)" stroke="#070b10" stroke-width=".6"></rect>')
    out.append(f'<symbol id="{p}hs" viewBox="0 0 20 20">{housing(20,20)}</symbol>')
    sizes = {'bem': (20, 40), 'fus': (40, 40), 'tor': (40, 40)}
    for k in ['fis', 'cap', 'shd', 'rad', 'cor', 'cgo', 'thr', 'pls', 'bem', 'fus', 'tor', 'acore']:
        w, h = sizes.get(k, (20, 20))
        out.append(f'<symbol id="{p}{k}" viewBox="0 0 {w} {h}">{housing(w,h)}{glyph_svg(p,k,w,h)}</symbol>')
    return "\n".join(out)

def slope_svg(p, poly, fill):
    pts = " ".join(f"{f(x)},{f(y)}" for x, y in poly)
    # hypotenuse = longest edge, highlight it slightly inset
    import math
    edges = [(poly[i], poly[(i + 1) % 3]) for i in range(3)]
    (x1, y1), (x2, y2) = max(edges, key=lambda e: math.dist(*e))
    cx = sum(x for x, _ in poly) / 3; cy = sum(y for _, y in poly) / 3
    def inset(x, y, d=1.4):
        dx, dy = cx - x, cy - y; L = math.hypot(dx, dy)
        return x + dx / L * d * 2.2, y + dy / L * d
    a, b = inset(x1, y1), inset(x2, y2)
    return (f'<polygon points="{pts}" fill="{fill}" stroke="#0a0f16"></polygon>'
            f'<path d="M{f(a[0])} {f(a[1])}L{f(b[0])} {f(b[1])}" stroke="#eef5fa" stroke-opacity=".26" stroke-width="1.2"></path>')

def extras(p, barrels, muzzles, plumes, glow=True):
    pl = ''
    for poly in plumes:
        pts = " ".join(f"{f(x)},{f(y)}" for x, y in poly)
        if glow:
            pl += f'<polygon points="{pts}" fill="#79d3a4" opacity=".55" filter="url(#{p}gl2)"></polygon>'
        pl += f'<polygon points="{pts}" fill="url(#{p}gPl)"></polygon>'
    br = ''.join(f'<rect x="{f(x)}" y="{f(y)}" width="{f(w)}" height="{f(h)}" rx="1" fill="url(#{p}gBar)" stroke="#0a0f16" stroke-width=".6"></rect>' for x, y, w, h in barrels)
    mz = ''
    for x, y in muzzles:
        if glow:
            mz += f'<circle cx="{f(x)}" cy="{f(y)}" r="3" fill="#5cc6d6" filter="url(#{p}gl)"></circle>'
        mz += f'<circle cx="{f(x)}" cy="{f(y)}" r="1.4" fill="#eafcff"></circle>'
    return pl, br, mz

def edit_ship(p, parts, barrels, muzzles, plumes):
    pl, br, mz = extras(p, barrels, muzzles, plumes)
    body = []
    for q in parts:
        if q['kind'] == 'slope':
            body.append(slope_svg(p, q['poly'], f'url(#{p}gA)'))
        else:
            body.append(f'<use href="#{p}{q["kind"]}" x="{q["x"]}" y="{q["y"]}" width="{q["w"]}" height="{q["h"]}"></use>')
    return pl + br + "".join(body) + mz

def skin_ship(p, parts, level, barrels, muzzles, plumes, seams=(), stripes=(), lights=()):
    hull = hull_geom(parts, level)
    hp = path_of(hull)
    pl, br, mz = extras(p, barrels, muzzles, plumes)
    out = [pl, br]
    out.append(f'<clipPath id="{p}clip"><path d="{hp}"></path></clipPath>')
    out.append(f'<path d="{hp}" fill="url(#{p}gHull)"></path>')
    clip = f'clip-path="url(#{p}clip)"'
    if level == 'A':
        lines = "".join(
            f'<polygon points="{" ".join(f"{f(x)},{f(y)}" for x, y in q["poly"])}"></polygon>' if q['poly']
            else f'<rect x="{q["x"]}" y="{q["y"]}" width="{q["w"]}" height="{q["h"]}"></rect>'
            for q in parts if q['kind'] in ('blk', 'slope'))
        out.append(f'<g {clip} fill="none" stroke="#0b1118" stroke-opacity=".6" stroke-width=".8">{lines}</g>')
    else:
        plates = PLATES_B if level == 'B' else PLATES_C
        if parts is MID:
            tones = "".join(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{'#ffffff' if t > 0 else '#000000'}" fill-opacity="{abs(t)}"></rect>' for x, y, w, h, t in plates if t)
            out.append(f'<g {clip}>{tones}{GREEBLES}</g>')
        d = "".join(seams)
        out.append(f'<g {clip} fill="none"><path d="{d}" stroke="#0a1016" stroke-opacity=".75" stroke-width=".9"></path>'
                   f'<path d="{d}" stroke="#eef5fa" stroke-opacity=".07" stroke-width=".8" transform="translate(.8 .8)"></path></g>')
    for q in parts:
        k = q['kind']
        if k not in MODULES:
            continue
        x, y, w, h = q['x'], q['y'], q['w'], q['h']
        if level == 'A':
            bay = (f'<rect x=".4" y=".4" width="{f(w-.8)}" height="{f(h-.8)}" fill="url(#{p}gH)" stroke="#0a0f16" stroke-width=".7"></rect>'
                   f'<rect x="2.6" y="2.6" width="{f(w-5.2)}" height="{f(h-5.2)}" rx="1.4" fill="url(#{p}gBay)"></rect>')
            out.append(f'<g transform="translate({x} {y})">{bay}{glyph_svg(p,k,w,h,glow_op=.5)}</g>')
        else:
            if k in ('thr', 'tor'):
                bay = f'<rect x="2.2" y="{f(h*.35)}" width="{f(w-4.4)}" height="{f(h*.65-1.2)}" rx="2.6" fill="url(#{p}gBay)" stroke="#060a0f" stroke-width=".7"></rect>'
                out.append(f'<g transform="translate({x} {y})">{bay}{fglyph(p,k,w,h)}</g>')
            elif k == 'acore':
                out.append(f'<g transform="translate({x} {y})">{glyph_svg(p,k,w,h)}</g>')
            else:
                out.append(f'<g transform="translate({x} {y})">{fglyph(p,k,w,h)}</g>')
    rim_op = '.5' if level == 'A' else '1'
    out.append(f'<path d="{hp}" fill="none" stroke="url(#{p}gRim)" stroke-width="2.6" opacity="{rim_op}" {clip}></path>')
    if stripes:
        out.append(f'<path d="{"".join(stripes)}" stroke="#e2b15c" stroke-opacity=".6" stroke-width="1.1" stroke-linecap="round" {clip}></path>')
    for x, y, col in lights:
        out.append(f'<circle cx="{x}" cy="{y}" r="3.2" fill="{col}" filter="url(#{p}gl)"></circle><circle cx="{x}" cy="{y}" r="1.2" fill="#fff"></circle>')
    out.append(f'<path d="{hp}" fill="none" stroke="#05080c" stroke-opacity=".85" stroke-width=".8"></path>')
    out.append(mz)
    return "".join(out)

def mid_skin(p, level, rot=0, mat='composite'):
    seams = MID_SEAMS_B if level == 'B' else MID_SEAMS_C
    st = MID_STRIPES if level in ('B', 'C') else ()
    li = MID_LIGHTS if level in ('B', 'C') else ()
    return defs(p, mat, rot), skin_ship(p, MID, level, MID_BARRELS, MID_MUZZLES, MID_PLUMES, seams, st, li)

def auto_skin(p, rot=0):
    return defs(p, 'steel', rot), skin_ship(p, AUTO, 'B', AUTO_BARRELS, AUTO_MUZZLES, AUTO_PLUMES, ["M-30 10H30"], (), ())

if __name__ == '__main__':
    for lv in 'ABC':
        d, s = mid_skin('t', lv)
        print(lv, len(d), len(s), path_of(hull_geom(MID, lv))[:120])
