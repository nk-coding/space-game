"""Chosen ship style '2a refined': flat tiles, one flat colour per machine tile, icon straight on it, soft glow."""
import sys, math
sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
from shapely.geometry import Polygon
from frag import T, SL, RT, RSL, glyph, f

ARM, MAC, SEAM, RARM = '#142a2e', '#0f1c25', '#03070a', '#221f2a'
TEAL, CORAL = '#8cefdb', '#ec8f7c'
E = "M100 0H140L180 20 220 40 240 60V80L220 100 200 120H180L160 140H80L60 120H40L20 100 0 80V60L20 40 60 20Z"
RE = "M-10 -30H10L30 -10V10L10 30H-10L-30 10V-10Z"
SIZES = {'fis': (20,20), 'cap': (20,20), 'shd': (20,20), 'rad': (20,20), 'cor': (20,20), 'cgo': (20,20), 'thr': (20,20),
         'pls': (20,20), 'bem': (20,40), 'fus': (40,40), 'tor': (40,40), 'acore': (20,20), 'cnn': (20,20)}
GLOW_OP = .55

def icon(k, w, h):
    g, c = glyph(k, w, h)
    c = c.replace('fill="#223032"', 'fill="none"').replace('fill="url(#gNoz)"', 'fill="#8cefdb" fill-opacity=".12"')
    return f'<g filter="url(#gl)" opacity="{GLOW_OP}">{g}</g>{c}' if g else c

def defs(p=''):
    """Gradients, filters and symbols. p prefixes every id (for several styles in one doc)."""
    out = [
     f'<radialGradient id="{p}gCore"><stop offset="0" stop-color="#fff4d6"></stop><stop offset=".4" stop-color="#f2c472" stop-opacity=".9"></stop><stop offset="1" stop-color="#e2b15c" stop-opacity="0"></stop></radialGradient>',
     f'<radialGradient id="{p}gCy"><stop offset="0" stop-color="#eafcff"></stop><stop offset=".45" stop-color="#5cc6d6" stop-opacity=".9"></stop><stop offset="1" stop-color="#5cc6d6" stop-opacity="0"></stop></radialGradient>',
     f'<radialGradient id="{p}gRed"><stop offset="0" stop-color="#ffe2da"></stop><stop offset=".45" stop-color="#ec8f7c" stop-opacity=".9"></stop><stop offset="1" stop-color="#ec8f7c" stop-opacity="0"></stop></radialGradient>',
     f'<linearGradient id="{p}gAmb" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f6d595"></stop><stop offset="1" stop-color="#a87a35"></stop></linearGradient>',
     f'<linearGradient id="{p}gBar" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#2a3a40"></stop><stop offset=".45" stop-color="#a6c4c4"></stop><stop offset=".7" stop-color="#456064"></stop><stop offset="1" stop-color="#1c272b"></stop></linearGradient>',
     f'<linearGradient id="{p}gPl" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#e6fff8" stop-opacity=".9"></stop><stop offset=".25" stop-color="#8cefdb" stop-opacity=".55"></stop><stop offset="1" stop-color="#8cefdb" stop-opacity="0"></stop></linearGradient>',
     f'<linearGradient id="{p}gPlR" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffe6df" stop-opacity=".9"></stop><stop offset=".3" stop-color="#ec8f7c" stop-opacity=".5"></stop><stop offset="1" stop-color="#ec8f7c" stop-opacity="0"></stop></linearGradient>',
     f'<radialGradient id="{p}gAura"><stop offset="0" stop-color="#8cefdb" stop-opacity=".1"></stop><stop offset=".55" stop-color="#8cefdb" stop-opacity=".04"></stop><stop offset="1" stop-color="#8cefdb" stop-opacity="0"></stop></radialGradient>',
     f'<radialGradient id="{p}gAuraR"><stop offset="0" stop-color="#ec8f7c" stop-opacity=".12"></stop><stop offset="1" stop-color="#ec8f7c" stop-opacity="0"></stop></radialGradient>',
     f'<filter id="{p}gl" x="-60%" y="-60%" width="220%" height="220%"><feGaussianBlur stdDeviation="1.6"></feGaussianBlur></filter>',
     f'<filter id="{p}gl2" x="-60%" y="-60%" width="220%" height="220%"><feGaussianBlur stdDeviation="3.2"></feGaussianBlur></filter>',
     f'<symbol id="{p}s-blk" viewBox="0 0 20 20"><rect x=".4" y=".4" width="19.2" height="19.2" fill="{ARM}" stroke="{SEAM}" stroke-width=".8"></rect></symbol>',
     f'<symbol id="{p}s-blkr" viewBox="0 0 20 20"><rect x=".4" y=".4" width="19.2" height="19.2" fill="{RARM}" stroke="{SEAM}" stroke-width=".8"></rect></symbol>',
    ]
    for k, (w, h) in SIZES.items():
        ic = icon(k, w, h)
        for a in ['gCore', 'gCy', 'gRed', 'gAmb', 'gl)']:
            ic = ic.replace(f'url(#{a}', f'url(#{p}{a}')
        out.append(f'<symbol id="{p}s-{k}" viewBox="0 0 {w} {h}"><rect x=".4" y=".4" width="{f(w-.8)}" height="{f(h-.8)}" fill="{MAC}" stroke="{SEAM}" stroke-width=".8"></rect>{ic}</symbol>')
    return "\n".join(out)

def slope_polys(polys, fill):
    return "".join(f'<polygon points="{" ".join(f"{x},{y}" for x, y in p)}" fill="{fill}" stroke="{SEAM}" stroke-width=".8"></polygon>' for p in polys)

def ship_body(p=''):
    return slope_polys(SL, ARM) + "".join(f'<use href="#{p}s-{k}" x="{x}" y="{y}" width="{w}" height="{h}"></use>' for k, x, y, w, h in T)

def plumes(p='', trails=True):
    out = (f'<g fill="#8cefdb" opacity=".3" filter="url(#{p}gl2)"><polygon points="44,119 56,119 50,142"></polygon><polygon points="184,119 196,119 190,142"></polygon><polygon points="105,139 135,139 120,180"></polygon></g>'
           f'<g fill="url(#{p}gPl)"><polygon points="44,119 56,119 50,142"></polygon><polygon points="184,119 196,119 190,142"></polygon><polygon points="105,139 135,139 120,180"></polygon></g>')
    if trails:
        dots = []
        for x in (50, 190):
            for i in range(1, 7):
                dots.append(f'<circle cx="{x}" cy="{f(142 + i*7.36)}" r="{f(max(.45, .78*(1-i/12)))}" opacity="{f(.6*(1-i/9.5))}"></circle>')
        for i in range(1, 8):
            dots.append(f'<circle cx="120" cy="{f(180 + i*12.4)}" r="{f(1.95*(1-i/12))}" opacity="{f(.6*(1-i/9.5))}"></circle>')
        out += f'<g fill="#8cefdb">{"".join(dots)}</g>'
    return out

def ship(p='', aura=True, engines=True, trails=True, shields=False, lights=True, glow=True):
    out = []
    if aura:
        out.append(f'<circle cx="120" cy="70" r="185" fill="url(#{p}gAura)"></circle>')
    if shields:
        out.append('<g fill="none" stroke="#8fb0f2" stroke-width="1.6" stroke-dasharray="0.1 4.5" stroke-linecap="round" opacity=".5"><circle cx="90" cy="51" r="46"></circle><circle cx="150" cy="51" r="46"></circle></g>')
    if engines:
        out.append(plumes(p, trails))
    out.append(f'<g fill="url(#{p}gBar)" stroke="#0a0f16" stroke-width=".6"><rect x="107.5" y="-14" width="5" height="16" rx="1"></rect><rect x="127.5" y="-14" width="5" height="16" rx="1"></rect></g>')
    out.append(ship_body(p))
    if glow:
        out.append(f'<path d="{E}" fill="none" stroke="#8cefdb" stroke-width="3" stroke-opacity=".18" filter="url(#{p}gl)"></path>')
        out.append(f'<path d="{E}" fill="none" stroke="#8cefdb" stroke-width=".9" stroke-opacity=".32"></path>')
    if lights:
        out.append(f'<circle cx="4" cy="77" r="3" fill="#ec8f7c" filter="url(#{p}gl)" opacity=".8"></circle><circle cx="4" cy="77" r="1.1" fill="#fff"></circle><circle cx="236" cy="77" r="3" fill="#8cefdb" filter="url(#{p}gl)" opacity=".8"></circle><circle cx="236" cy="77" r="1.1" fill="#fff"></circle>')
    out.append(f'<g fill="#5cc6d6" filter="url(#{p}gl)" opacity=".85"><circle cx="110" cy="-14" r="3"></circle><circle cx="130" cy="-14" r="3"></circle><circle cx="10" cy="61" r="2.4"></circle><circle cx="230" cy="61" r="2.4"></circle></g>'
               '<g fill="#eafcff"><circle cx="110" cy="-14" r="1.3"></circle><circle cx="130" cy="-14" r="1.3"></circle><circle cx="10" cy="61" r="1"></circle><circle cx="230" cy="61" r="1"></circle></g>')
    return "".join(out)

def raider(p=''):
    return (f'<circle cx="0" cy="0" r="44" fill="url(#{p}gAuraR)"></circle>'
            f'<polygon points="-6,29 6,29 0,46" fill="#ec8f7c" opacity=".3" filter="url(#{p}gl2)"></polygon><polygon points="-6,29 6,29 0,46" fill="url(#{p}gPlR)"></polygon>'
            '<g fill="#ec8f7c"><circle cx="0" cy="52" r=".9" opacity=".5"></circle><circle cx="0" cy="58" r=".7" opacity=".35"></circle><circle cx="0" cy="64" r=".5" opacity=".2"></circle></g>'
            f'<rect x="-2" y="-42" width="4" height="14" rx="1" fill="url(#{p}gBar)" stroke="#0a0f16" stroke-width=".6"></rect>'
            + slope_polys(RSL, RARM)
            + "".join(f'<use href="#{p}s-{"blkr" if k == "blk" else k}" x="{x}" y="{y}" width="{w}" height="{h}"></use>' for k, x, y, w, h in RT)
            + f'<path d="{RE}" fill="none" stroke="#ec8f7c" stroke-width="3" stroke-opacity=".2" filter="url(#{p}gl)"></path>'
            f'<path d="{RE}" fill="none" stroke="#ec8f7c" stroke-width=".9" stroke-opacity=".38"></path>'
            f'<circle cx="0" cy="-42" r="3" fill="#ec8f7c" filter="url(#{p}gl)"></circle><circle cx="0" cy="-42" r="1.2" fill="#ffe1d9"></circle>')

def tile_icon(k, size=28, p=''):
    """Palette icon: one part symbol in a small svg."""
    w, h = SIZES.get(k, (20, 20))
    if k == 'blk':
        return f'<svg width="{size}" height="{size}" viewBox="0 0 20 20" aria-hidden="true"><use href="#{p}s-blk" width="20" height="20"></use></svg>'
    vb = f"0 0 {w} {h}"
    return f'<svg width="{size}" height="{size}" viewBox="{vb}" aria-hidden="true"><use href="#{p}s-{k}" width="{w}" height="{h}"></use></svg>'
