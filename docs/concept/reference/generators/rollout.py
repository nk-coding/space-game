import sys, re; sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
import style2a as S

UI = '/tmp/claude-0/-home-claude/8726918e-2de4-5522-8295-f17b8d1e9e92/scratchpad/ui/project/'
OPT = '/tmp/claude-0/-home-claude/8726918e-2de4-5522-8295-f17b8d1e9e92/scratchpad/styleopts/project/'
BG = "radial-gradient(ellipse 75% 85% at 50% 45%, #0a1820, #050e15 55%, #010409)"
T, GOLD, CO, VI, LBL, DIM, WH = '#8cefdb', '#eed595', '#ec8f7c', '#ae8cec', '#869099', '#5d6a74', '#ebf5ff'
RGB = {T: '140, 239, 219', GOLD: '238, 213, 149', CO: '236, 143, 124', VI: '174, 140, 236', LBL: '134, 144, 153', WH: '235, 245, 255'}

def head(title, w, h):
    return f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Barlow:wght@400;500;600&amp;display=swap">
<style>
body{{margin:0}}
a{{color:#8cefdb}}a:hover{{color:#c8fff2}}
button{{font:inherit;color:inherit;cursor:pointer}}
</style>
</helmet>
'''
def tail(w, h):
    return ('</x-dc>\n<script type="text/x-dc" data-dc-script data-props=\'{"$preview":{"width":%d,"height":%d}}\'>\n'
            'class Component extends DCLogic {\nrenderVals() {\nreturn {};\n}\n}\n</script>\n</body>\n</html>\n') % (w, h)

def lab(t, col=LBL, size=9.5, sp=.3, extra=''):
    return f'<span style="font-size: {size}px; font-weight: 500; letter-spacing: {sp}em; color: {col}{"; " + extra if extra else ""}">{t}</span>'
def num(t, col=WH, size=15, extra=''):
    return f'<span style="font-size: {size}px; font-weight: 600; line-height: 1; color: {col}; text-shadow: 0 0 12px rgba({RGB[col]}, 0.35){"; " + extra if extra else ""}">{t}</span>'
def pill(t, on=False, col=T, h=28, pad=11, size=9.5, extra='', aria=''):
    a = f' aria-label="{aria}"' if aria else ''
    pressed = ' aria-pressed="true"' if on else ''
    if on:
        st = f"background: rgba({RGB[col]}, 0.12); border: 1px solid rgba({RGB[col]}, 0.7); box-shadow: 0 0 10px rgba({RGB[col]}, 0.25); color: #e6fff8"
    else:
        st = "background: transparent; border: 1px solid rgba(134, 144, 153, 0.3); color: #869099"
    return f'<button type="button"{a}{pressed} style="height: {h}px; padding: 0 {pad}px; border-radius: {h//2}px; {st}; font-size: {size}px; font-weight: 600; letter-spacing: 0.2em; white-space: nowrap{"; " + extra if extra else ""}">{t}</button>'
def primary(t, h=36, extra=''):
    return (f'<button type="button" style="height: {h}px; padding: 0 20px; border-radius: {h//2}px; display: flex; align-items: center; gap: 9px; background: radial-gradient(ellipse at 50% 50%, rgba(140, 239, 219, 0.2), rgba(140, 239, 219, 0.06)); '
            f'border: 1.5px solid rgba(140, 239, 219, 0.85); box-shadow: 0 0 16px rgba(140, 239, 219, 0.3), inset 0 0 10px rgba(140, 239, 219, 0.15); color: #e6fff8; font-size: 11px; font-weight: 600; letter-spacing: 0.24em{"; " + extra if extra else ""}">'
            f'<svg width="11" height="12" viewBox="0 0 11 12" aria-hidden="true"><path d="M1.5 1.5 9.5 6l-8 4.5z" fill="#8cefdb"></path></svg>{t}</button>')
def circ(svg, aria, size=34, col=LBL):
    return (f'<button type="button" aria-label="{aria}" style="width: {size}px; height: {size}px; border-radius: 50%; padding: 0; display: grid; place-items: center; background: transparent; border: 1px solid rgba({RGB[col]}, 0.4)">{svg}</button>')
def bar(pct, col, w='100%'):
    return f'<div style="width: {w}; height: 2px; border-radius: 1px; background: rgba(134, 144, 153, 0.18)"><div style="width: {pct}%; height: 2px; border-radius: 1px; background: {col}; box-shadow: 0 0 6px {col}"></div></div>'

RING_ICON = '<svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6" fill="none" stroke="#ae8cec" stroke-width="1.3" stroke-dasharray="0.1 2.6" stroke-linecap="round"></circle><circle cx="8" cy="8" r="2" fill="#ae8cec"></circle></svg>'
UNDO = '<svg width="15" height="15" viewBox="0 0 18 18" aria-hidden="true"><path d="M6 4 2 8l4 4M2.5 8H11a5 5 0 0 1 0 10H8" fill="none" stroke="#b9c4cc" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"></path></svg>'
REDO = '<svg width="15" height="15" viewBox="0 0 18 18" aria-hidden="true"><path d="M12 4l4 4-4 4M15.5 8H7a5 5 0 0 0 0 10h3" fill="none" stroke="#b9c4cc" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"></path></svg>'

def dots_pattern(pid, cell):
    return f'<pattern id="{pid}" width="{cell}" height="{cell}" patternUnits="userSpaceOnUse"><circle cx="0" cy="0" r="1" fill="#1d2d35"></circle><circle cx="{cell}" cy="0" r="1" fill="#1d2d35"></circle><circle cx="0" cy="{cell}" r="1" fill="#1d2d35"></circle><circle cx="{cell}" cy="{cell}" r="1" fill="#1d2d35"></circle></pattern>'

def slope_icon(pts, vb):
    return f'<svg width="24" height="24" viewBox="{vb}" aria-hidden="true"><polygon points="{pts}" fill="{S.ARM}" stroke="#8cefdb" stroke-opacity=".5" stroke-width="1"></polygon></svg>'

# ---------------------------------------------------------------- flight
def flight():
    src = open(OPT + 'R2Flat.dc.html').read()
    extra = re.search(r'(<radialGradient id="gHalo">.*?</radialGradient>\n<radialGradient id="gPearl".*?</radialGradient>\n<radialGradient id="gImpact">.*?</radialGradient>)', src, re.S).group(1)
    filt = '<filter id="glow" x="-20%" y="-50%" width="140%" height="200%"><feGaussianBlur stdDeviation="2.6"></feGaussianBlur></filter>\n<filter id="soft" x="-100%" y="-100%" width="300%" height="300%"><feGaussianBlur stdDeviation="2.2"></feGaussianBlur></filter>'
    new_defs = f'<defs>\n{S.defs()}\n{extra}\n{filt}\n<g id="raider">{S.raider()}</g>\n</defs>'
    src = re.sub(r'<defs>\n<linearGradient id="cA">.*?</defs>', lambda m: new_defs, src, count=1, flags=re.S)
    src = re.sub(r'(<g transform="translate\(312 145\) rotate\(28 84 49\) scale\(0.7\)">).*?(\n</g>\n\n<use href="#raider")',
                 lambda m: m.group(1) + S.ship(shields=True) + m.group(2), src, count=1, flags=re.S)
    src = src.replace('<title>Style 2a, flat: flight screen</title>', '<title>Flight HUD on a phone</title>')
    src = src.replace('aria-label="Style 2a: flat dark blocks with thin seams and a soft teal glow around the hull; the frigate fires both beams at a locked raider"',
                      'aria-label="In flight in an ion storm field: the block-built frigate fires both beams at a locked automaton raider; two more raiders shoot back; salvage orbs drift nearby"')
    assert 'cA' not in src.split('</defs>')[0][-200:]
    open(UI + 'Flight.dc.html', 'w').write(src)

# ---------------------------------------------------------------- phone drydock
def drydock_phone():
    sc, x0, y0 = 1.15, 274, 122
    cell = 20 * sc
    ghost = ('<g opacity=".9"><rect x="240.5" y="60.5" width="39" height="39" fill="rgba(140, 239, 219, 0.05)" stroke="#8cefdb" stroke-dasharray="3 3" stroke-width="1"></rect>'
             '<use href="#s-cgo" x="240" y="60" width="20" height="20" opacity=".45"></use><use href="#s-cgo" x="260" y="60" width="20" height="20" opacity=".45"></use>'
             '<use href="#s-cgo" x="240" y="80" width="20" height="20" opacity=".45"></use><use href="#s-cgo" x="260" y="80" width="20" height="20" opacity=".45"></use>'
             '<circle cx="268" cy="88" r="11" fill="rgba(235, 245, 255, 0.08)" stroke="rgba(235, 245, 255, 0.35)"></circle></g>')
    canvas = f'''<svg width="504" height="274" viewBox="160 60 504 274" role="img" aria-label="The frigate on the drydock grid, a cargo hold being dragged onto its right wing" style="position: absolute; left: 160px; top: 60px">
<defs>{dots_pattern('dd', cell)}</defs>
<rect x="160" y="60" width="504" height="274" fill="url(#dd)" transform="translate({x0 % cell:.1f} {y0 % cell:.1f})"></rect>
<path d="M{x0 + 120*sc:.1f} 64V330" stroke="#243540" stroke-dasharray="3 5"></path>
<g transform="translate({x0} {y0}) scale({sc})">{S.ship(aura=True, engines=False, glow=True)}{ghost}<circle cx="120" cy="72" r="3.5" fill="#ebf5ff" stroke="#050e15"></circle></g>
<g fill="none" stroke="#8cefdb" stroke-opacity=".5" stroke-width="1.2"><path d="M168 76v-8h8M656 76v-8h-8M168 318v8h8M656 318v8h-8"></path></g>
</svg>'''
    def item(icon, name, on=False, dashed=False):
        st = ("background: rgba(140, 239, 219, 0.08); box-shadow: inset 0 0 0 1px rgba(140, 239, 219, 0.45)" if on else
              "background: transparent; box-shadow: inset 0 0 0 1px rgba(140, 239, 219, 0.35)" if dashed else "background: transparent")
        border = "border: 1px dashed rgba(140, 239, 219, 0.6)" if dashed else "border: 0"
        return f'<button type="button"{" aria-pressed=\"true\"" if on else ""} style="height: 30px; display: flex; align-items: center; gap: 9px; padding: 0 8px; border-radius: 15px; {st}; {border}; text-align: left; font-size: 12.5px; color: {"#e6fff8" if on else "#b9c4cc"}">{icon}{name}</button>'
    pal = f'''<nav aria-label="Parts" style="position: absolute; left: 12px; top: 62px; width: 140px; display: flex; flex-direction: column; gap: 1px">
{lab("HULL", DIM, 9, .3, "padding: 2px 8px")}
{item(S.tile_icon('blk', 22), 'Block')}
{item(slope_icon("40,1 40,19 1,19", "0 0 40 20"), 'Slope 1×2', on=True)}
{item(slope_icon("60,5 60,19 1,19", "0 0 60 20"), 'Slope 1×3')}
{lab("POWER", DIM, 9, .3, "padding: 6px 8px 2px")}
{item(S.tile_icon('fis', 22), 'Fission')}
{item(S.tile_icon('fus', 22), 'Fusion 2×2')}
{item(S.tile_icon('cap', 22), 'Capacitor')}
{lab("CARGO", DIM, 9, .3, "padding: 6px 8px 2px")}
{item(S.tile_icon('cgo', 22), 'Hold 2×2', dashed=True)}
</nav>'''
    def stat(l, v, col=WH, b=None):
        bb = bar(*b) if b else ''
        return f'<div style="display: grid; gap: 5px"><div style="display: flex; justify-content: space-between; align-items: baseline">{lab(l, LBL, 9, .24)}{num(v, col, 14)}</div>{bb}</div>'
    read = f'''<aside aria-label="Ship readout" style="position: absolute; right: 16px; top: 66px; width: 150px; display: grid; gap: 11px">
{stat("POWER", "44 / 58", T, (76, GOLD))}
{stat("PEAK HEAT", "91 %", CO, (91, CO))}
{stat("THRUST/MASS", "0.78", T)}
{stat("YAW", "Balanced", T)}
{stat("FIREPOWER", "8.3×")}
{stat("CARGO", "8 → 28")}
{stat("MIND", "45 / 60", VI, (75, VI))}
{stat("DEVICE LOAD", "Light", T)}
</aside>'''
    hdr = f'''<div style="position: absolute; left: 14px; top: 12px; display: flex; align-items: center; gap: 12px">
{circ(RING_ICON, "Open the ring menu", 36, VI)}
<div style="display: grid; gap: 3px">{lab("DRYDOCK · FRIGATE FRAME", DIM, 9, .3)}<span style="font-size: 16px; font-weight: 600; color: #ebf5ff">Halvex runner</span></div>
</div>
<div style="position: absolute; right: 16px; top: 16px; display: flex; align-items: center; gap: 6px">
<div role="group" aria-label="Armor material" style="display: flex; gap: 4px">{pill("STEEL")}{pill("COMPOSITE", True)}{pill("REACTIVE")}</div>
<span style="width: 1px; height: 18px; background: rgba(134, 144, 153, 0.25); margin: 0 6px"></span>
<div role="group" aria-label="Overlays" style="display: flex; gap: 4px">{pill("HEAT")}{pill("MASS", True)}{pill("BONUS", True, GOLD)}</div>
</div>'''
    chip = f'<div style="position: absolute; left: 470px; top: 292px; display: grid; gap: 4px">{lab("HOLD 2×2 · DROP TO PLACE", "#c8fff2", 9, .24)}{lab("4 STEEL PLATE IN STORAGE", T, 9, .24, "opacity: 0.75")}</div>'
    ftr = f'''<div style="position: absolute; left: 14px; right: 16px; bottom: 12px; display: flex; align-items: center; gap: 8px">
{circ(UNDO, "Undo")}{circ(REDO, "Redo")}
<span style="width: 6px"></span>{pill("MIRROR", True)}{pill("BEVEL")}
<span style="margin-left: 10px">{lab("FLIGHT-READY · HEAT RUNNING WARM", T, 9.5, .24, "text-shadow: 0 0 8px rgba(140, 239, 219, 0.4)")}</span>
<span style="flex: 1"></span>{primary("TEST-FLY")}
</div>'''
    html = head('Drydock on a phone', 844, 390) + f'''<div style="width: 844px; height: 390px; box-sizing: border-box; background: {BG}; color: #ebf5ff; font-family: Barlow, 'Segoe UI', sans-serif; position: relative; overflow: hidden">
<svg width="0" height="0" style="position: absolute" aria-hidden="true"><defs>
{S.defs()}
</defs></svg>
{canvas}
{hdr}
{pal}
{read}
{chip}
{ftr}
</div>
''' + tail(844, 390)
    open(UI + 'Main.dc.html', 'w').write(html)

# ---------------------------------------------------------------- desktop drydock
def drydock_desktop():
    sc, x0, y0 = 2.3, 336, 214
    cell = 20 * sc
    sel = ('<rect x="100" y="40" width="40" height="40" fill="#8cefdb" fill-opacity=".06"></rect>'
           '<path d="M97 46v-9h9M134 37h9v9M143 74v9h-9M106 83h-9v-9" fill="none" stroke="#8cefdb" stroke-width="2.2" opacity=".45" filter="url(#gl)"></path>'
           '<path d="M97 46v-9h9M134 37h9v9M143 74v9h-9M106 83h-9v-9" fill="none" stroke="#c8fff2" stroke-width=".9"></path>'
           '<g fill="#eed595" stroke="#050e15" stroke-width=".5"><path d="M110 37.5l2.5 2.5-2.5 2.5-2.5-2.5z"></path><path d="M130 37.5l2.5 2.5-2.5 2.5-2.5-2.5z"></path></g>'
           '<circle cx="120" cy="72" r="2.4" fill="#ebf5ff" stroke="#050e15" stroke-width=".8"></circle>')
    canvas = f'''<svg width="656" height="580" viewBox="284 72 656 580" role="img" aria-label="The frigate on the drydock grid at 230 percent, the fusion reactor selected" style="position: absolute; left: 284px; top: 72px">
<defs>{dots_pattern('dk', cell)}</defs>
<rect x="284" y="72" width="656" height="580" fill="url(#dk)" transform="translate({x0 % cell:.1f} {y0 % cell:.1f})"></rect>
<path d="M{x0 + 120*sc:.1f} 76V648" stroke="#243540" stroke-dasharray="4 6"></path>
<g transform="translate({x0} {y0}) scale({sc})">{S.ship(aura=True, engines=False)}{sel}</g>
<g fill="none" stroke="#8cefdb" stroke-opacity=".5" stroke-width="1.2"><path d="M294 90v-10h10M930 90v-10h-10M294 634v10h10M930 634v10h-10"></path></g>
</svg>'''
    tools = f'<div role="toolbar" aria-label="Build tools" style="position: absolute; left: 612px; top: 84px; transform: translateX(-50%); display: flex; gap: 6px">{pill("PAINT B", True)}{pill("FILL G")}{pill("ERASE X")}{pill("SELECT V")}{pill("ROTATE Q E")}</div>'
    canvas_info = (f'<div style="position: absolute; left: 304px; top: 626px">{lab("CELL 5,2 · MIRROR ON · AXIS 6 · SPACE + DRAG TO PAN", DIM, 9, .24)}</div>'
                   f'<div role="group" aria-label="Zoom" style="position: absolute; left: 812px; top: 616px; display: flex; align-items: center; gap: 6px">{circ("<span style=\"font-size: 15px; color: #b9c4cc\">−</span>", "Zoom out", 28)}{num("230 %", WH, 12)}{circ("<span style=\"font-size: 15px; color: #b9c4cc\">+</span>", "Zoom in", 28)}</div>')
    hdr = f'''<div style="position: absolute; left: 24px; top: 14px; display: flex; align-items: center; gap: 14px">
{circ(RING_ICON, "Open the ring menu", 40, VI)}
<div style="display: grid; gap: 4px">{lab("SHIPYARD › DRYDOCK · FRIGATE FRAME", DIM, 9.5, .3)}<span style="font-size: 18px; font-weight: 600; color: #ebf5ff">Halvex runner</span></div>
</div>
<div style="position: absolute; right: 24px; top: 20px; display: flex; align-items: center; gap: 8px">
{lab("ARMOR", DIM, 9, .3)}<div role="group" aria-label="Armor material" style="display: flex; gap: 4px">{pill("STEEL 1")}{pill("COMPOSITE 2", True)}{pill("REACTIVE 3")}</div>
<span style="width: 1px; height: 18px; background: rgba(134, 144, 153, 0.25); margin: 0 8px"></span>
{lab("OVERLAY", DIM, 9, .3)}<div role="group" aria-label="Overlays" style="display: flex; gap: 4px">{pill("HEAT")}{pill("MASS", True)}{pill("BONUS", True, GOLD)}{pill("SHIELDS")}{pill("ARCS")}</div>
</div>'''
    def row(icon, name, sub, status, scol, on=False, locked=False):
        st = "background: rgba(140, 239, 219, 0.08); box-shadow: inset 0 0 0 1px rgba(140, 239, 219, 0.45)" if on else "background: transparent"
        return (f'<button type="button"{" aria-pressed=\"true\"" if on else ""} style="height: 46px; display: grid; grid-template-columns: 34px 1fr auto; align-items: center; column-gap: 10px; padding: 0 12px; border-radius: 23px; border: 0; {st}; text-align: left{"; opacity: 0.55" if locked else ""}">'
                f'{icon}<span style="display: grid; gap: 3px"><span style="font-size: 13.5px; color: {"#e6fff8" if on else "#d4dde4"}">{name}</span>{lab(sub, DIM, 8.5, .2)}</span>{lab(status, scol, 8.5, .2)}</button>')
    locked_icon = '<svg width="30" height="30" viewBox="0 0 30 30" aria-hidden="true"><rect x="5.5" y="2.5" width="19" height="25" rx="2" fill="none" stroke="#5d6a74" stroke-dasharray="2.5 2.5"></rect><path d="M12 14v-2.5a3 3 0 0 1 6 0V14M11 14h8v6h-8z" fill="none" stroke="#869099" stroke-width="1.2"></path></svg>'
    bio_icon = '<svg width="30" height="30" viewBox="0 0 30 30" aria-hidden="true"><rect x="5.5" y="5.5" width="19" height="19" rx="2" fill="none" stroke="#5d6a74" stroke-dasharray="2.5 2.5"></rect><circle cx="15" cy="15" r="5" fill="none" stroke="#a8e6a1" stroke-opacity=".7"></circle></svg>'
    sink_icon = '<svg width="30" height="30" viewBox="0 0 20 40" aria-hidden="true"><use href="#s-rad" width="20" height="20"></use><use href="#s-rad" y="20" width="20" height="20"></use></svg>'
    pal = f'''<nav aria-label="Parts" style="position: absolute; left: 20px; top: 76px; width: 248px; display: flex; flex-direction: column; gap: 8px">
<div style="height: 34px; display: flex; align-items: center; gap: 9px; padding: 0 14px; border-radius: 17px; border: 1px solid rgba(134, 144, 153, 0.3); font-size: 13px; color: #5d6a74"><svg width="13" height="13" viewBox="0 0 14 14" aria-hidden="true"><circle cx="6" cy="6" r="4.2" fill="none" stroke="#869099" stroke-width="1.3"></circle><path d="M9.2 9.2 13 13" stroke="#869099" stroke-width="1.3"></path></svg><span style="flex: 1">Search parts</span>{lab("/", DIM, 10, .1)}</div>
<div role="tablist" aria-label="Part categories" style="display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 5px">{pill("HULL", h=26, pad=6, size=8.5)}{pill("POWER", True, h=26, pad=6, size=8.5)}{pill("WEAPONS", h=26, pad=6, size=8.5)}{pill("DEFENSE", h=26, pad=6, size=8.5)}{pill("DRIVE", h=26, pad=6, size=8.5)}{pill("UTILITY", h=26, pad=6, size=8.5)}</div>
<span style="padding: 6px 12px 0">{lab("POWER AND HEAT · 7", DIM, 9, .3)}</span>
{row(S.tile_icon('fis', 30), 'Fission cell', '1×1 · +6 POWER', '2 SPARE', T)}
{row(S.tile_icon('fus', 30), 'Fusion reactor', '2×2 · +24 POWER', 'CRAFT', GOLD, on=True)}
{row(S.tile_icon('cap', 30), 'Capacitor', '1×1 · BUFFER 40', '4 SPARE', T)}
{row(S.tile_icon('rad', 30), 'Radiator', '1×1 · −1.2 HEAT/S', '3 SPARE', T)}
{row(sink_icon, 'Heat sink', '1×2 · STORES 30', 'CRAFT', GOLD)}
{row(locked_icon, 'Antimatter core', '2×3 · INNER BAND', 'LOCKED', DIM, locked=True)}
{row(bio_icon, 'Grown biocell', '1×1 · GROWN TRUST', 'LOCKED', DIM, locked=True)}
</nav>'''
    def st(l, v, col=WH, b=None):
        return f'<div style="display: grid; gap: 5px"><div style="display: flex; justify-content: space-between; align-items: baseline">{lab(l, LBL, 9, .22)}{num(v, col, 14)}</div>{bar(*b) if b else ""}</div>'
    def mat(dot, name, have, hcol):
        return (f'<span style="width: 8px; height: 8px; border-radius: 50%; background: {dot}; box-shadow: 0 0 6px {dot}"></span>'
                f'<span style="font-size: 13px; color: #d4dde4">{name}</span>{lab(have, hcol, 9, .2)}')
    right = f'''<aside aria-label="Ship readout and part inspector" style="position: absolute; right: 24px; top: 80px; width: 284px; display: grid; gap: 18px">
<div style="display: grid; grid-template-columns: 1fr 1fr; column-gap: 22px; row-gap: 12px">
{st("POWER", "44 / 58", T, (76, GOLD))}{st("PEAK HEAT", "91 %", CO, (91, CO))}
{st("THRUST/MASS", "0.78", T)}{st("YAW", "Balanced", T)}
{st("FIREPOWER", "8.3×")}{st("CARGO", "8 cells")}
{st("MIND", "45 / 60", VI, (75, VI))}{st("DEVICE", "Light", T)}
</div>
<div style="height: 1px; background: rgba(140, 239, 219, 0.08)"></div>
<div style="display: grid; gap: 14px">
{lab("SELECTED PART", DIM, 9, .3)}
<div style="display: flex; align-items: center; gap: 14px">{S.tile_icon('fus', 46)}<div style="display: grid; gap: 4px"><span style="font-size: 18px; font-weight: 600; color: #ebf5ff">Fusion reactor</span>{lab("2×2 · CELL 5,2 · INSTALLED", DIM, 8.5, .22)}</div></div>
<div style="display: grid; gap: 7px">{lab("VARIANT", DIM, 9, .3)}<div role="group" aria-label="Variant" style="display: flex; gap: 5px">{pill("STANDARD", h=26, pad=9, size=8.5)}{pill("COMPACT TORUS", True, h=26, pad=9, size=8.5)}{pill("HOT-BURN", h=26, pad=9, size=8.5, extra="opacity: 0.5")}</div></div>
<div style="display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px">
<div style="display: grid; gap: 5px">{lab("POWER", LBL, 8.5, .2)}<span style="font-size: 13.5px; font-weight: 600">24 <span style="color: #8cefdb">→ 30</span></span></div>
<div style="display: grid; gap: 5px">{lab("HEAT/S", LBL, 8.5, .2)}<span style="font-size: 13.5px; font-weight: 600">2.6 <span style="color: #ec8f7c">→ 3.1</span></span></div>
<div style="display: grid; gap: 5px">{lab("MASS", LBL, 8.5, .2)}<span style="font-size: 13.5px; font-weight: 600">8.0 <span style="color: #8cefdb">→ 7.2</span></span></div>
<div style="display: grid; gap: 5px">{lab("HULL", LBL, 8.5, .2)}<span style="font-size: 13.5px; font-weight: 600">140</span></div>
</div>
<div style="display: flex; align-items: center; gap: 8px"><svg width="9" height="9" viewBox="0 0 10 10" aria-hidden="true"><path d="M5 0l5 5-5 5-5-5z" fill="#eed595"></path></svg>{lab("FEEDS 2 BEAM LASERS DIRECTLY · +6 % OUTPUT", GOLD, 8.5, .16, "text-shadow: 0 0 8px rgba(238, 213, 149, 0.35)")}</div>
</div>
<div style="height: 1px; background: rgba(140, 239, 219, 0.08)"></div>
<div style="display: grid; gap: 10px">
<div style="display: flex; justify-content: space-between">{lab("UPGRADE MATERIALS", DIM, 9, .3)}{lab("BONDED ONCE", DIM, 8.5, .2)}</div>
<div style="display: grid; grid-template-columns: 10px 1fr auto; column-gap: 10px; row-gap: 8px; align-items: center">
{mat(GOLD, "Helion cell ×2", "5 STORED", T)}
{mat("#8d9db4", "Steel plate ×4", "31 STORED", T)}
{mat(VI, "Lattice coil ×1", "NONE", CO)}
<span></span><span style="grid-column: 2 / -1; margin-top: -3px">{lab("SALVAGE FROM LATTICE SPIRES · MIDDLE BAND", LBL, 8, .16)}</span>
</div>
<div style="display: flex; gap: 8px; margin-top: 4px">{pill("UPGRADE · 1 MISSING", h=34, pad=14, size=9, extra="opacity: 0.55; flex: 1")}{pill("TRACK ON MAP", True, VI, h=34, pad=14, size=9, extra="flex: 1")}</div>
</div>
</aside>'''
    ftr = f'''<div style="position: absolute; left: 24px; right: 24px; bottom: 16px; display: flex; align-items: center; gap: 8px">
{circ(UNDO, "Undo", 36)}{circ(REDO, "Redo", 36)}<span style="width: 8px"></span>{pill("MIRROR M", True, h=32)}{pill("BEVEL SHIFT B", h=32)}
<span style="margin-left: 12px">{lab("FLIGHT-READY · HEAT RUNNING WARM", T, 9.5, .24, "text-shadow: 0 0 8px rgba(140, 239, 219, 0.4)")}</span>
<span style="flex: 1"></span>{pill("BLUEPRINTS", h=32)}{pill("EXPORT", h=32)}{pill("SAVE CTRL S", h=32)}<span style="width: 4px"></span>{primary("TEST-FLY T", 38)}
</div>'''
    html = head('Drydock on desktop', 1280, 720) + f'''<div style="width: 1280px; height: 720px; box-sizing: border-box; background: {BG}; color: #ebf5ff; font-family: Barlow, 'Segoe UI', sans-serif; position: relative; overflow: hidden">
<svg width="0" height="0" style="position: absolute" aria-hidden="true"><defs>
{S.defs()}
</defs></svg>
{canvas}
{tools}
{canvas_info}
{hdr}
{pal}
{right}
{ftr}
</div>
''' + tail(1280, 720)
    open(UI + 'Desktop.dc.html', 'w').write(html)

# ---------------------------------------------------------------- home ring menu
def home():
    sc, x0, y0 = .95, 308, 128
    scene = f'''<svg width="844" height="390" viewBox="0 0 844 390" aria-hidden="true" style="position: absolute; inset: 0">
<defs><radialGradient id="hHalo"><stop offset="0" stop-color="#8cefdb" stop-opacity=".07"></stop><stop offset="1" stop-color="#8cefdb" stop-opacity="0"></stop></radialGradient></defs>
<ellipse cx="422" cy="190" rx="300" ry="170" fill="url(#hHalo)"></ellipse>
<g fill="none" stroke="#414c52" stroke-width="1.4" stroke-dasharray="0.1 6" stroke-linecap="round"><ellipse cx="422" cy="190" rx="250" ry="140"></ellipse><rect x="284" y="104" width="276" height="176" rx="4" opacity=".7"></rect></g>
<g fill="#2c4a4a"><circle cx="284" cy="104" r="2"></circle><circle cx="560" cy="104" r="2"></circle><circle cx="284" cy="280" r="2"></circle><circle cx="560" cy="280" r="2"></circle></g>
<g fill="#eed595"><circle cx="284" cy="104" r="1.2"></circle><circle cx="560" cy="104" r="1.2"></circle><circle cx="284" cy="280" r="1.2"></circle><circle cx="560" cy="280" r="1.2"></circle></g>
<g transform="translate({x0} {y0}) scale({sc})">{S.ship(aura=True, engines=False)}</g>
</svg>'''
    def node(x, y, size, col, icon, name, sub=None, sub_col=None, lx=None, ly=None, align='left', big=False, aria=None):
        rgb = RGB[col]
        b = (f'<button type="button" aria-label="{aria or name}" style="position: absolute; left: {x}px; top: {y}px; width: {size}px; height: {size}px; border-radius: 50%; padding: 0; display: grid; place-items: center; '
             f'background: radial-gradient(circle, rgba({rgb}, {0.2 if big else 0.08}), rgba({rgb}, 0.02) 70%); border: 1.5px solid rgba({rgb}, 0.85); box-shadow: 0 0 {22 if big else 14}px rgba({rgb}, {0.4 if big else 0.25}), inset 0 0 10px rgba({rgb}, 0.14)">{icon}</button>')
        sub_html = f'{lab(sub, sub_col or col, 8.5, .22)}' if sub else ''
        ta = 'text-align: right; justify-items: end;' if align == 'right' else ''
        t = f'<div aria-hidden="true" style="position: absolute; left: {lx}px; top: {ly}px; display: grid; gap: 3px; {ta}">{lab(name, "#e6fff8" if big else "#b9c4cc", 10 if big else 9.5, .26)}{sub_html}</div>'
        return b + t
    def ic(path, col, s=20):
        return f'<svg width="{s}" height="{s}" viewBox="0 0 20 20" aria-hidden="true">{path.replace("COL", col)}</svg>'
    nodes = "".join([
        node(396, 22, 52, WH, ic('<path d="M3 3h14v14H3zM3 10h14M10 3v14" fill="none" stroke="COL" stroke-width="1.3"></path>', '#ebf5ff'), 'DRYDOCK', lx=458, ly=40),
        node(573, 63, 52, T, ic('<ellipse cx="10" cy="10" rx="8" ry="3.5" fill="none" stroke="COL" stroke-width="1.3" transform="rotate(-30 10 10)"></ellipse><circle cx="10" cy="10" r="2" fill="COL"></circle>', T), 'RESEARCH', '3 READY', lx=634, ly=74),
        node(646, 164, 52, GOLD, ic('<path d="M4 3h12v5H4zM10 8v6M6 14h8v3H6z" fill="none" stroke="COL" stroke-width="1.3"></path>', GOLD), 'FABRICATOR', '2 IN QUEUE', lx=708, ly=175),
        node(573, 265, 52, CO, ic('<circle cx="10" cy="10" r="7" fill="none" stroke="COL" stroke-width="1.3"></circle><circle cx="10" cy="10" r="3.5" fill="none" stroke="COL" stroke-width="1.3"></circle><circle cx="10" cy="10" r="1" fill="COL"></circle>', CO), 'TEST RANGE', lx=634, ly=284),
        node(388, 296, 68, T, ic('<path d="M13 3l8 18-8-5-8 5z" fill="none" stroke="COL" stroke-width="1.6" stroke-linejoin="round"></path>', T, 26).replace('viewBox="0 0 20 20"', 'viewBox="0 0 26 26"'), 'LAUNCH VOYAGE', 'HALVEX DRIFT', '#8cefdb', lx=466, ly=316, big=True),
        node(219, 265, 52, LBL, ic('<path d="M3 12h6v5H3zM11 12h6v5h-6zM7 5h6v5H7z" fill="none" stroke="COL" stroke-width="1.3"></path>', '#b9c4cc'), 'STORAGE', lx=142, ly=284, align='right'),
        node(146, 164, 52, '#8cefdb' if False else LBL, ic('<path d="M4 15 8 6l6 3 3-5" fill="none" stroke="COL" stroke-width="1.3"></path><circle cx="4" cy="15" r="1.8" fill="COL"></circle><circle cx="8" cy="6" r="1.8" fill="COL"></circle><circle cx="14" cy="9" r="1.8" fill="COL"></circle><circle cx="17" cy="4" r="1.8" fill="COL"></circle>', '#8fb0f2'), 'GALAXY MAP', lx=54, ly=182, align='right'),
        node(219, 63, 52, VI, ic('<path d="M10 3v4M10 7 4 12M10 7l6 5M4 12v4M16 12v4" fill="none" stroke="COL" stroke-width="1.3"></path><rect x="8" y="2" width="4" height="3" fill="COL"></rect><rect x="2" y="15" width="4" height="3" fill="COL"></rect><rect x="14" y="15" width="4" height="3" fill="COL"></rect>', VI), 'BEHAVIOR', lx=146, ly=80, align='right'),
    ])
    info = f'''<div style="position: absolute; left: 18px; top: 14px; display: grid; gap: 4px">{lab("THE SHIPYARD · RIM", DIM, 9, .3)}<span style="font-size: 16px; font-weight: 600; color: #ebf5ff">Halvex runner</span></div>
<div style="position: absolute; right: 18px; top: 14px; width: 170px; display: grid; gap: 5px; justify-items: end; text-align: right">{lab("RESEARCHING", DIM, 9, .3)}<span style="font-size: 13px; color: #d4dde4">Beam laser · focused</span><div style="display: flex; align-items: center; gap: 8px">{bar(62, T, "118px")}{num("62 %", T, 11, "white-space: nowrap")}</div></div>
<div style="position: absolute; left: 18px; bottom: 14px; display: grid; gap: 4px">{lab("HAUL UNLOADED", DIM, 9, .3)}<span style="font-size: 13px; color: #d4dde4">14 parts · 9 materials</span></div>'''
    html = head('Home ring menu on a phone', 844, 390) + f'''<div style="width: 844px; height: 390px; box-sizing: border-box; background: {BG}; color: #ebf5ff; font-family: Barlow, 'Segoe UI', sans-serif; position: relative; overflow: hidden">
<svg width="0" height="0" style="position: absolute" aria-hidden="true"><defs>
{S.defs()}
</defs></svg>
{scene}
{info}
{nodes}
</div>
''' + tail(844, 390)
    open(UI + 'Home.dc.html', 'w').write(html)

if __name__ == '__main__':
    flight(); drydock_phone(); drydock_desktop(); home()
    import os
    for n in ['Flight', 'Main', 'Desktop', 'Home']:
        print(n, os.path.getsize(UI + n + '.dc.html'))
