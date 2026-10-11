# Drydock rules

How the drydock decides what a design can do. The numbers live in the part pages; these are the rules.

- **Pieces.** Parts that touch through straight sides form a piece. [[part:slope_1x1|Slopes]] connect only
  through their two straight sides, never through the slope face. Each piece has its own power pool; a piece
  that breaks off becomes salvage.
- **Power.** Reactors add to the pool, everything else draws from it. Short on power, every consumer in the
  piece runs at the same reduced share.
- **Heat.** Parts heat up under load and pass heat to linked neighbours. [[part:radiator|Radiators]] vent
  through sides open to space. Overheated parts shut down.
- **Adjacency bonuses.** [[part:capacitor|Capacitors]] make lasers beside them fire faster; venting radiators
  beside a reactor raise its output.
- **Guns and engines.** Guns need a clear line of fire ahead; engines need open cells directly behind them.
