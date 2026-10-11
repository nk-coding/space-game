# Generators (reference only)

The Python scripts that produced the style 2a ship SVGs and the UI mockup boards. They need Python 3 and `shapely`.

- `style2a.py`: the chosen ship style. Builds the SVG defs (gradients, glow filters, one symbol per part), plus `ship()`, `raider()` and `plumes()`. The values here are the source of `assets/tokens.json`.
- `frag.py`: the sample ship layouts. `T` holds the tiles and `SL` the slopes of the 12×7 Halvex Runner (20 px per cell); `RT` and `RSL` do the same for the raider. **Importing it writes `syms.txt`, `body.txt` and similar scratch files into the current directory.**
- `skins_v2.py`: the original icon glyphs that `frag.glyph()` recolours.
- `rollout.py`, `rollout2.py`: generated the mockup boards. Their output paths point at the old design-canvas folder, so they won't run as-is here.

For the game, use the finished SVGs in `assets/` rather than running these.
