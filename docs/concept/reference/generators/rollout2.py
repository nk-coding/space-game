import sys, random, math; sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
import io, contextlib
with contextlib.redirect_stdout(io.StringIO()):
    from rollout import *
import style2a as S

def particles(seed, n, w, h, x0=0):
    r = random.Random(seed)
    return '<g fill="#6c9b95">' + "".join(f'<circle cx="{x0 + r.uniform(0, w):.0f}" cy="{r.uniform(0, h):.0f}" r="{r.choice([.5, .6, .8, 1])}" opacity="{r.uniform(.12, .5):.2f}"></circle>' for _ in range(n)) + '</g>'

def switch(label, on=True):
    knob = 'right: 2px; background: #8cefdb; box-shadow: 0 0 8px #8cefdb' if on else 'left: 2px; background: #5d6a74'
    track = 'background: rgba(140, 239, 219, 0.18); border: 1px solid rgba(140, 239, 219, 0.7)' if on else 'background: transparent; border: 1px solid rgba(134, 144, 153, 0.4)'
    return (f'<button type="button" role="switch" aria-checked="{"true" if on else "false"}" style="display: flex; align-items: center; justify-content: space-between; gap: 10px; width: 100%; padding: 0; background: transparent; border: 0; text-align: left">'
            f'{lab(label, "#b9c4cc", 9, .2)}<span aria-hidden="true" style="position: relative; flex: none; width: 30px; height: 16px; border-radius: 8px; box-sizing: border-box; {track}"><span style="position: absolute; top: 2px; width: 10px; height: 10px; border-radius: 50%; {knob}"></span></span></button>')

def chip(t, col):
    return f'<span style="height: 20px; display: inline-flex; align-items: center; padding: 0 9px; border-radius: 10px; border: 1px solid rgba({RGB[col]}, 0.5); background: rgba({RGB[col]}, 0.07); font-size: 8.5px; font-weight: 600; letter-spacing: 0.2em; color: {col}">{t}</span>'

# ---------------------------------------------------------------- galaxy map
def galaxy_map():
    N = dict(H=(190,230), R1=(170,120), R2=(200,330), R3=(250,60), R4=(250,372), O1=(330,165), O2=(310,270), O3=(350,75), O4=(360,320),
             M1=(440,200), M2=(460,100), M3=(455,290), I1=(540,160), I2=(560,260), G=(403,188))
    def ln(a, b): return f'M{N[a][0]} {N[a][1]} {N[b][0]} {N[b][1]}'
    charted = "".join(ln(a, b) for a, b in [('H','R1'),('H','R2'),('R1','R3'),('R3','O3'),('R1','O1'),('O1','O2'),('R2','O2'),('R2','R4'),('R4','O4'),('O2','O4'),('O1','G')])
    unknown = "".join(ln(a, b) for a, b in [('G','M1'),('O3','M2'),('O4','M3'),('M1','I1'),('M3','I2'),('M1','M2'),('M1','M3')])
    route = f'M190 230 330 165 350 75'
    svg = f'''<svg width="600" height="390" viewBox="0 0 600 390" role="img" aria-label="Galaxy map: the rim and outer bands are charted, a guardian gate leads to the middle band; a route runs from the shipyard via Corran Reach to Halvex Drift" style="position: absolute; left: 0; top: 0">
<defs><filter id="mGl" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="2.2"></feGaussianBlur></filter></defs>
<g fill="#8cefdb"><circle cx="700" cy="195" r="560" fill-opacity=".022"></circle><circle cx="700" cy="195" r="430" fill-opacity=".018"></circle><circle cx="700" cy="195" r="300" fill-opacity=".02"></circle><circle cx="700" cy="195" r="190" fill-opacity=".025"></circle></g>
{particles(5, 45, 600, 390)}
<g fill="none" stroke-width="1.4" stroke-dasharray="0.1 6" stroke-linecap="round"><circle cx="700" cy="195" r="560" stroke="#414c52"></circle><circle cx="700" cy="195" r="430" stroke="#414c52"></circle><circle cx="700" cy="195" r="300" stroke="#ec8f7c" stroke-opacity=".55"></circle><circle cx="700" cy="195" r="190" stroke="#414c52"></circle></g>
<g font-family="Barlow, sans-serif" font-size="9" font-weight="500" letter-spacing="2.6" fill="#5d6a74" text-anchor="middle"><text x="233" y="30">RIM</text><text x="374" y="30">OUTER</text><text x="519" y="30">MIDDLE</text><text x="574" y="208">INNER</text></g>
<path d="{charted}" fill="none" stroke="#8cefdb" stroke-opacity=".22" stroke-width="1.2"></path>
<path d="{unknown}" fill="none" stroke="#414c52" stroke-width="1.1" stroke-dasharray="3 4"></path>
<path d="{route}" fill="none" stroke="#8cefdb" stroke-width="5" stroke-opacity=".28" stroke-linejoin="round" filter="url(#mGl)"></path>
<path d="{route}" fill="none" stroke="#c8fff2" stroke-width="2.2" stroke-dasharray="0.1 6" stroke-linecap="round" stroke-linejoin="round"></path>
<g font-family="Barlow, sans-serif" font-size="8.5" font-weight="600" letter-spacing="1.8" fill="#8cefdb"><text x="250" y="186" text-anchor="middle">WARP 0:40</text><text x="352" y="126">WARP 0:55</text></g>
<g fill="#2e514d"><circle cx="440" cy="200" r="2.6"></circle><circle cx="460" cy="100" r="2.6"></circle><circle cx="455" cy="290" r="2.6"></circle><circle cx="540" cy="160" r="2.6"></circle><circle cx="560" cy="260" r="2.6"></circle></g>
<g fill="#8cefdb" opacity=".55" filter="url(#mGl)"><circle cx="170" cy="120" r="5"></circle><circle cx="200" cy="330" r="5"></circle></g>
<g fill="#8cefdb" fill-opacity=".85"><circle cx="170" cy="120" r="4"></circle><circle cx="200" cy="330" r="4"></circle></g>
<g fill="#050e15" stroke="#8cefdb" stroke-opacity=".8" stroke-width="1.3"><circle cx="250" cy="60" r="4.2"></circle><circle cx="250" cy="372" r="4.2"></circle><circle cx="330" cy="165" r="4.2"></circle><circle cx="310" cy="270" r="4.2"></circle><circle cx="360" cy="320" r="4.2"></circle></g>
<circle cx="350" cy="75" r="16" fill="none" stroke="#8cefdb" stroke-width="1.4" stroke-dasharray="0.1 4.5" stroke-linecap="round" opacity=".8"></circle>
<circle cx="350" cy="75" r="7" fill="#8cefdb" opacity=".45" filter="url(#mGl)"></circle><circle cx="350" cy="75" r="5.5" fill="#050e15" stroke="#c8fff2" stroke-width="1.6"></circle><circle cx="350" cy="75" r="1.8" fill="#c8fff2"></circle>
<path d="M403 179l9 9-9 9-9-9z" fill="#ec8f7c" opacity=".35" filter="url(#mGl)"></path><path d="M403 180l8 8-8 8-8-8z" fill="#050e15" stroke="#ec8f7c" stroke-width="1.4"></path>
<path d="M250 39l4 4-4 4-4-4z" fill="#ae8cec" filter="url(#mGl)" opacity=".6"></path><path d="M250 39l4 4-4 4-4-4z" fill="#ae8cec"></path>
<path d="M190 219l11 11-11 11-11-11z" fill="#ebf5ff" opacity=".25" filter="url(#mGl)"></path><path d="M190 220l10 10-10 10-10-10z" fill="#050e15" stroke="#ebf5ff" stroke-width="1.4"></path><path d="M190 225l3 7h-6z" fill="#8cefdb"></path>
<g font-family="Barlow, sans-serif" font-weight="500" letter-spacing="2.2">
<text x="176" y="234" font-size="9" fill="#d4dde4" text-anchor="end">SHIPYARD</text>
<text x="342" y="157" font-size="8.5" fill="#869099">CORRAN REACH</text>
<text x="372" y="71" font-size="10" font-weight="600" fill="#e6fff8">HALVEX DRIFT</text>
<text x="403" y="215" font-size="8.5" fill="#ec8f7c" text-anchor="middle">GUARDIAN GATE</text>
<text x="262" y="44" font-size="8.5" fill="#ae8cec">CONTRACT</text>
</g>
</svg>'''
    hdr = f'''<div style="position: absolute; left: 14px; top: 12px; display: flex; align-items: center; gap: 12px">{circ(RING_ICON, "Open the ring menu", 36, VI)}<div style="display: grid; gap: 3px">{lab("82 CLUSTERS · 17 CHARTED · 6 CLEARED", DIM, 9, .26)}<span style="font-size: 16px; font-weight: 600; color: #ebf5ff">Galaxy map</span></div></div>'''
    def leg(svg, t): return f'<span style="display: flex; align-items: center; gap: 7px">{svg}{lab(t, LBL, 8.5, .22)}</span>'
    legend = f'''<div style="position: absolute; left: 16px; bottom: 14px; display: grid; grid-template-columns: auto auto; column-gap: 16px; row-gap: 6px">
{leg('<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1l4 4-4 4-4-4z" fill="none" stroke="#ebf5ff" stroke-width="1.1"></path></svg>', 'HOME')}
{leg('<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><circle cx="5" cy="5" r="3.5" fill="#8cefdb" fill-opacity=".85"></circle></svg>', 'CLEARED')}
{leg('<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><circle cx="5" cy="5" r="3.4" fill="none" stroke="#8cefdb" stroke-width="1.1"></circle></svg>', 'CHARTED')}
{leg('<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><circle cx="5" cy="5" r="2.4" fill="#2e514d"></circle></svg>', 'UNKNOWN')}
{leg('<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1l4 4-4 4-4-4z" fill="#ae8cec"></path></svg>', 'CONTRACT')}
{leg('<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1l4 4-4 4-4-4z" fill="none" stroke="#ec8f7c" stroke-width="1.1"></path></svg>', 'GATE')}
</div>'''
    overlay = f'<div role="group" aria-label="Map overlay" style="position: absolute; left: 380px; bottom: 14px; display: flex; gap: 5px">{pill("LANES", True, h=26, pad=10, size=8.5)}{pill("MATERIALS", h=26, pad=10, size=8.5)}{pill("THREAT", h=26, pad=10, size=8.5)}</div>'
    def rrow(svg, name, t, col): return f'{svg}<span style="font-size: 12.5px; color: #d4dde4">{name}</span>{lab(t, col, 8.5, .2)}'
    def supply(n, pct, col, val, marker=None):
        mk = f'<div style="position: absolute; left: {marker}%; top: -3px; width: 1px; height: 8px; background: #ebf5ff"></div>' if marker else ''
        return (f'{lab(n, LBL, 8.5, .2)}<div style="position: relative; height: 2px; border-radius: 1px; background: rgba(134, 144, 153, 0.18)"><div style="width: {pct}%; height: 2px; border-radius: 1px; background: {col}; box-shadow: 0 0 6px {col}"></div>{mk}</div>'
                f'<span style="text-align: right">{lab(val, col, 8.5, .18)}</span>')
    panel = f'''<div aria-hidden="true" style="position: absolute; left: 600px; top: 18px; bottom: 18px; width: 1px; background: rgba(140, 239, 219, 0.08)"></div>
<aside aria-label="Voyage plan" style="position: absolute; left: 618px; right: 16px; top: 14px; bottom: 12px; display: flex; flex-direction: column; gap: 9px">
<div style="display: grid; gap: 6px">{lab("DESTINATION", DIM, 9, .3)}<span style="font-size: 18px; font-weight: 600; color: #ebf5ff; text-shadow: 0 0 12px rgba(235, 245, 255, 0.25)">Halvex Drift</span><div style="display: flex; flex-wrap: wrap; gap: 4px">{chip("OUTER BAND", LBL)}{chip("AUTOMATONS", CO)}{chip("ION STORMS", T)}</div></div>
<div style="display: grid; gap: 7px">{lab("ROUTE", DIM, 9, .3)}
<div style="display: grid; grid-template-columns: 12px 1fr auto; column-gap: 8px; row-gap: 4px; align-items: center">
{rrow('<svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1l5 5-5 5-5-5z" fill="none" stroke="#ebf5ff" stroke-width="1.1"></path></svg>', 'Shipyard', 'DOCKED', DIM)}
{rrow('<svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><circle cx="6" cy="6" r="3.8" fill="none" stroke="#8cefdb" stroke-opacity=".7" stroke-width="1.1"></circle></svg>', 'Corran Reach', 'WARP 0:40', T)}
{rrow('<svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><circle cx="6" cy="6" r="3.8" fill="none" stroke="#c8fff2" stroke-width="1.5"></circle></svg>', 'Halvex Drift', 'WARP 0:55', T)}
</div>{lab("VOYAGE ≈ 20 MIN · RETURN ANY TIME", LBL, 8.5, .18)}</div>
<div style="display: grid; grid-template-columns: 52px 1fr 52px; column-gap: 8px; row-gap: 6px; align-items: center">
{supply("FUEL", 100, T, "54 %", 46)}
{supply("AMMO", 100, GOLD, "FULL")}
{supply("REPAIR", 100, "#a8e6a1" if False else T, "6 / 6")}
</div>
<div style="display: grid; gap: 6px">{switch("AUTOPILOT IN TRANSIT")}{switch("HAND OVER ON LOCK")}</div>
<span style="margin-top: auto">{primary("SET COURSE", 36, "width: 100%; justify-content: center; white-space: nowrap")}</span>
</aside>'''
    html = head('Galaxy map and autopilot on a phone', 844, 390) + f'''<div style="width: 844px; height: 390px; box-sizing: border-box; background: {BG}; color: #ebf5ff; font-family: Barlow, 'Segoe UI', sans-serif; position: relative; overflow: hidden">
{svg}
{hdr}
{legend}
{overlay}
{panel}
</div>
''' + tail(844, 390)
    open(UI + 'Map.dc.html', 'w').write(html)

# ---------------------------------------------------------------- behavior editor
def behavior():
    def cond(t, grey=False):
        if grey:
            return f'<span style="height: 30px; display: inline-flex; align-items: center; padding: 0 12px; border-radius: 15px; border: 1px solid rgba(134, 144, 153, 0.4); font-size: 11.5px; color: #b9c4cc">{t}</span>'
        return f'<span style="height: 30px; display: inline-flex; align-items: center; padding: 0 12px; border-radius: 15px; border: 1px solid rgba(174, 140, 236, 0.6); background: rgba(174, 140, 236, 0.08); font-size: 11.5px; color: #e6dcfb">{t}</span>'
    def act(t, sub, sel=False, rhai=False):
        st = ("border: 1.5px solid #8cefdb; background: rgba(140, 239, 219, 0.14); box-shadow: 0 0 14px rgba(140, 239, 219, 0.35)" if sel
              else "border: 1px solid rgba(140, 239, 219, 0.6); background: rgba(140, 239, 219, 0.07)")
        tag = f'<span style="font-size: 8px; font-weight: 600; letter-spacing: 0.2em; color: #eed595">RHAI</span>' if rhai else ''
        return f'<span style="height: 30px; display: inline-flex; align-items: center; gap: 7px; padding: 0 12px; border-radius: 15px; {st}{"; border-style: dashed" if rhai and not sel else ""}; font-size: 11.5px; color: #e6fff8">{t}<span style="color: #869099">{sub}</span>{tag}</span>'
    def wire(w): return f'<span aria-hidden="true" style="width: {w}px; height: 1px; background: rgba(140, 239, 219, 0.3)"></span>'
    ys = [80, 132, 184, 236, 288]
    rows = [
        cond("shield < 30%") + wire(118) + act("Retreat to", "leader"),
        cond("missile incoming") + cond("PD ready") + wire(20) + act("Point defense", "missiles"),
        cond("enemy attacks leader") + wire(50) + act("Flank", "left · 350", rhai=True),
        cond("wreck in range") + cond("no enemies") + wire(28) + act("Tractor", "nearest wreck", sel=True),
        cond("always", grey=True) + wire(160) + act("Hold slot", "left wing"),
    ]
    row_html = "".join(f'<div style="position: absolute; left: 140px; top: {y}px; display: flex; align-items: center; gap: 8px">{r}</div>' for y, r in zip(ys, rows))
    wires = (f'<div aria-hidden="true" style="position: absolute; left: 124px; top: {ys[0]+15}px; width: 1px; height: {ys[-1]-ys[0]}px; background: rgba(140, 239, 219, 0.3)"></div>'
             f'<div aria-hidden="true" style="position: absolute; left: 106px; top: {ys[2]+15}px; width: 18px; height: 1px; background: rgba(140, 239, 219, 0.3)"></div>'
             + "".join(f'<div aria-hidden="true" style="position: absolute; left: 124px; top: {y+15}px; width: 16px; height: 1px; background: rgba(140, 239, 219, 0.3)"></div>'
                       f'<div aria-hidden="true" style="position: absolute; left: 122px; top: {y+13}px; width: 5px; height: 5px; border-radius: 50%; background: #8cefdb; box-shadow: 0 0 6px #8cefdb"></div>' for y in ys))
    root = f'<div style="position: absolute; left: 14px; top: {ys[2]-6}px; width: 92px; height: 42px; box-sizing: border-box; padding: 0 12px; border-radius: 21px; border: 1px solid rgba(235, 245, 255, 0.55); display: grid; align-content: center; gap: 2px"><span style="font-size: 12.5px; font-weight: 600; color: #ebf5ff">Priority</span>{lab("FIRST MATCH", DIM, 7.5, .2)}</div>'
    add_rule = f'<div style="position: absolute; left: 140px; top: 340px">{pill("+ RULE · 1 SLOT LEFT", h=28, pad=12, size=8.5, extra="border-style: dashed")}</div>'
    zoom = f'<div role="group" aria-label="Zoom" style="position: absolute; left: 556px; top: 296px; display: grid; gap: 6px">{circ("<span style=\"font-size: 15px; color: #b9c4cc\">+</span>", "Zoom in", 32)}{circ("<span style=\"font-size: 15px; color: #b9c4cc\">−</span>", "Zoom out", 32)}</div>'
    grid_bg = f'<svg width="610" height="330" viewBox="0 0 610 330" aria-hidden="true" style="position: absolute; left: 0; top: 60px"><defs>{dots_pattern("bg", 18)}</defs><rect width="610" height="330" fill="url(#bg)"></rect></svg>'
    hdr = f'''<div style="position: absolute; left: 14px; top: 12px; display: flex; align-items: center; gap: 12px">{circ(RING_ICON, "Open the ring menu", 36, VI)}<div style="display: grid; gap: 3px">{lab("BEHAVIOR · ESCORT DRONE", DIM, 9, .3)}<span style="font-size: 16px; font-weight: 600; color: #ebf5ff">Escort, left wing</span></div><span style="margin-left: 6px">{chip("5 / 6 RULE SLOTS", VI)}</span></div>
<div style="position: absolute; right: 16px; top: 15px; display: flex; align-items: center; gap: 8px"><div role="group" aria-label="View" style="display: flex; gap: 4px">{pill("GRAPH", True)}{pill("TEXT")}</div><span style="width: 6px"></span>{primary("TEST IN RANGE", 32)}</div>'''
    sel_style = "height: 34px; width: 100%; padding: 0 12px; border-radius: 17px; background: transparent; color: #e6fff8; border: 1px solid rgba(140, 239, 219, 0.5); font-family: Barlow, sans-serif; font-size: 12.5px"
    insp = f'''<div aria-hidden="true" style="position: absolute; left: 618px; top: 64px; bottom: 16px; width: 1px; background: rgba(140, 239, 219, 0.08)"></div>
<aside aria-label="Selected node" style="position: absolute; left: 634px; right: 16px; top: 68px; display: grid; gap: 12px">
<div style="display: grid; gap: 4px">{lab("ACTION · MANEUVER", DIM, 9, .3)}<span style="font-size: 18px; font-weight: 600; color: #8cefdb; text-shadow: 0 0 12px rgba(140, 239, 219, 0.35)">Tractor</span></div>
<label style="display: grid; gap: 6px">{lab("TARGET", LBL, 8.5, .24)}<select style="{sel_style}"><option>nearest wreck</option><option>largest wreck</option><option>marked wreck</option></select></label>
<label style="display: grid; gap: 4px">{lab("LEASH TO LEADER · 600 M", LBL, 8.5, .2)}<input type="range" min="100" max="1500" value="600" style="width: 100%; accent-color: #8cefdb"></label>
{switch("DROP IT WHEN ENEMIES APPEAR")}
<div style="height: 1px; background: rgba(140, 239, 219, 0.08)"></div>
<div style="display: grid; gap: 7px">{lab("ADD NODE", DIM, 9, .3)}<div style="display: flex; flex-wrap: wrap; gap: 5px">{chip("HEAT > 80 %", VI)}{chip("CARGO FULL", VI)}{chip("ORBIT", T)}{chip("KEEP RANGE", T)}{chip("CUSTOM · RHAI", GOLD)}</div></div>
</aside>'''
    html = head('Behavior editor on a phone', 844, 390) + f'''<div style="width: 844px; height: 390px; box-sizing: border-box; background: {BG}; color: #ebf5ff; font-family: Barlow, 'Segoe UI', sans-serif; position: relative; overflow: hidden">
{grid_bg}
{hdr}
<main aria-label="Behavior graph">
{wires}
{root}
{row_html}
{add_rule}
{zoom}
</main>
{insp}
</div>
''' + tail(844, 390)
    open(UI + 'Behavior.dc.html', 'w').write(html)

# ---------------------------------------------------------------- parts style sheet
def parts_sheet():
    def tile_svg(inner, vb_w, vb_h, box=72):
        sc = min(box / vb_w, box / vb_h) * (1 if max(vb_w, vb_h) > 20 else .9)
        w, h = vb_w * sc, vb_h * sc
        return f'<svg width="{w:.0f}" height="{h:.0f}" viewBox="0 0 {vb_w} {vb_h}" aria-hidden="true" style="filter: drop-shadow(0 0 10px rgba(140, 239, 219, 0.12))">{inner}</svg>'
    def card(svg, name, tag):
        return (f'<div style="width: 112px; height: 124px; box-sizing: border-box; border-radius: 10px; border: 1px solid rgba(140, 239, 219, 0.07); background: radial-gradient(ellipse at 50% 35%, #0b1a21, #060f15 75%); display: grid; grid-template-rows: 1fr auto; justify-items: center; padding: 10px 6px">'
                f'<div style="display: grid; place-items: center">{svg}</div><div style="display: grid; gap: 3px; justify-items: center; text-align: center"><span style="font-size: 12px; color: #d4dde4">{name}</span>{lab(tag, DIM, 8, .2)}</div></div>')
    def part(k, name, tag):
        w, h = S.SIZES[k]
        return card(tile_svg(f'<use href="#s-{k}" width="{w}" height="{h}"></use>', w, h), name, tag)
    def mat(col, name):
        return card(tile_svg(f'<rect x=".4" y=".4" width="19.2" height="19.2" fill="{col}" stroke="{S.SEAM}" stroke-width=".8"></rect>', 20, 20), name, 'ARMOR 1×1')
    def slope(pts, w, name):
        return card(tile_svg(f'<polygon points="{pts}" fill="{S.ARM}" stroke="{S.SEAM}" stroke-width=".8"></polygon>', w, 20), name, f'SLOPE 1×{w // 20}')
    def group(title, cards):
        return f'<div style="display: grid; gap: 7px">{lab(title, DIM, 9, .3)}<div style="display: flex; gap: 10px">{"".join(cards)}</div></div>'
    rows = [
        [group("ARMOR MATERIALS", [mat('#1b2531', 'Steel'), mat(S.ARM, 'Composite'), mat('#211d2c', 'Reactive')]),
         group("SLOPES", [slope("0,20 20,0 20,20", 20, 'Slope'), slope("0,20 40,0 40,20", 40, 'Slope'), slope("0,20 60,0 60,20", 60, 'Slope')])],
        [group("POWER AND HEAT", [part('fis', 'Fission cell', '1×1'), part('fus', 'Fusion reactor', '2×2'), part('cap', 'Capacitor', '1×1'), part('rad', 'Radiator', '1×1')]),
         group("DEFENSE AND CONTROL", [part('shd', 'Shield emitter', '1×1'), part('cor', 'Logic core', '1×1')])],
        [group("CARGO", [part('cgo', 'Cargo bay', '1×1')]),
         group("PROPULSION", [part('thr', 'Thruster', '1×1'), part('tor', 'Torch drive', '2×2')]),
         group("WEAPONS", [part('pls', 'Pulse turret', '1×1'), part('bem', 'Beam laser', '1×2')]),
         group("AUTOMATON", [part('acore', 'Automaton core', 'ENEMY')])],
    ]
    left = "".join(f'<div style="display: flex; gap: 22px">{"".join(r)}</div>' for r in rows)
    ship_svg = f'<svg width="430" height="330" viewBox="-30 -30 300 230" role="img" aria-label="The frigate built from these parts, in flight with engines on" style="display: block">{S.ship(aura=True, engines=True, trails=True, shields=False)}</svg>'
    html = head('Ship parts style sheet', 1280, 600) + f'''<div style="width: 1280px; height: 600px; box-sizing: border-box; padding: 24px 28px; background: {BG}; color: #ebf5ff; font-family: Barlow, 'Segoe UI', sans-serif; position: relative; overflow: hidden; display: grid; grid-template-columns: auto 1fr; column-gap: 28px">
<svg width="0" height="0" style="position: absolute" aria-hidden="true"><defs>
{S.defs()}
</defs></svg>
<div style="display: grid; gap: 16px; align-content: start">
<div style="display: grid; gap: 4px">{lab("SHIP PARTS · STYLE SHEET", DIM, 9.5, .32)}<span style="font-size: 20px; font-weight: 600; color: #ebf5ff">Flat tiles, one colour per machine, icon on the tile</span></div>
{left}
</div>
<div style="display: grid; align-content: center; justify-items: center; gap: 10px">{ship_svg}{lab("HALVEX RUNNER · 12 × 7 CELLS · SAME TILES IN DRYDOCK AND FLIGHT", LBL, 8.5, .22)}</div>
</div>
''' + tail(1280, 600)
    open(UI + 'Skins.dc.html', 'w').write(html)

if __name__ == '__main__':
    galaxy_map(); behavior(); parts_sheet()
    import os
    for n in ['Map', 'Behavior', 'Skins']:
        print(n, os.path.getsize(UI + n + '.dc.html'))
