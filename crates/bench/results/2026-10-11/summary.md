- Native (VM), 1 thread, one process per run: 162 runs, 1-min load average during runs 3.6–12.3 (median 7.3), threads 1
- Native (VM), 1 thread, lockstep (all engines in one process, ticking in turn): 162 runs, 1-min load average during runs 1.3–8.3 (median 2.0), threads 1
- Native (VM), 1 thread, lockstep, sleeping disabled: 72 runs, 1-min load average during runs 3.9–13.2 (median 7.8), threads 1
- Native (VM), multi-thread: 135 runs, 1-min load average during runs 4.7–13.3 (median 9.4), threads 4
- wasm in Node (V8), 1 thread: 135 runs, 1-min load average during runs 2.7–10.1 (median 6.4), threads 1

### Step time, Native (VM), 1 thread, one process per run (ms per tick, median / p95)

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---:|---:|---:|---:|---:|---:|
| ships | 8.85 / 25.7 | 1.06 / 3.17 | 25.4 / 58.4 | 28.6 / 68.8 | 0.04 / 0.13 | 26.3 / 58.7 |
| ships-merged | 0.39 / 1.04 | 0.07 / 0.15 | 0.81 / 1.89 | 1.16 / 3.87 | 0.03 / 0.08 | 0.65 / 1.20 |
| debris | 1.21 / 3.19 | 0.80 / 1.84 | 2.50 / 6.05 | 5.63 / 13.3 | 2.09 / 6.74 | 3.20 / 8.07 |
| raycasts | 15.0 / 75.6 | 5.66 / 18.9 | 33.7 / 117 | 48.7 / 103 | 3.76 / 15.0 | 38.2 / 82.5 |
| breakage | 11.0 / 30.4 | 0.90 / 4.02 | 20.8 / 55.0 | 30.0 / 48.3 | 0.10 / 0.42 | 25.3 / 45.1 |
| breakage-merged | 0.41 / 0.85 | 0.10 / 0.20 | 0.93 / 1.73 | 1.40 / 4.32 | 0.06 / 0.17 | 0.83 / 1.64 |
| asteroids | 0.07 / 0.18 | 0.08 / 0.19 | 0.08 / 0.23 | 0.33 / 0.78 | 0.19 / 0.44 | 0.08 / 0.14 |
| combined | 22.6 / 48.7 | 3.89 / 13.4 | 46.6 / 82.4 | 43.6 / 88.2 | 2.27 / 7.71 | 48.1 / 90.9 |
| combined-merged | 4.32 / 18.8 | 2.64 / 12.4 | 6.77 / 18.8 | 12.3 / 30.4 | 4.11 / 14.5 | 5.47 / 14.0 |

### Step time, Native (VM), 1 thread, lockstep (all engines in one process, ticking in turn) (ms per tick, median / p95)

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---:|---:|---:|---:|---:|---:|
| ships | 7.25 / 12.2 | 1.12 / 2.03 | 16.7 / 25.5 | 20.3 / 31.3 | 0.55 / 1.43 | 16.8 / 25.6 |
| ships-merged | 0.29 / 0.54 | 0.06 / 0.18 | 0.56 / 0.86 | 0.83 / 1.47 | 0.06 / 0.24 | 0.55 / 0.82 |
| debris | 1.50 / 4.22 | 1.26 / 3.92 | 2.69 / 5.13 | 4.40 / 8.31 | 2.06 / 4.95 | 2.38 / 3.48 |
| raycasts | 12.0 / 21.7 | 4.12 / 8.76 | 22.6 / 37.9 | 27.8 / 39.4 | 4.20 / 8.80 | 23.2 / 37.2 |
| breakage | 8.20 / 16.0 | 1.20 / 2.98 | 15.5 / 24.8 | 19.3 / 32.4 | 0.49 / 1.45 | 15.6 / 26.5 |
| breakage-merged | 0.28 / 0.62 | 0.07 / 0.17 | 0.51 / 0.75 | 0.79 / 1.35 | 0.07 / 0.19 | 0.50 / 0.72 |
| asteroids | 0.06 / 0.11 | 0.06 / 0.11 | 0.06 / 0.11 | 0.19 / 0.30 | 0.11 / 0.19 | 0.06 / 0.10 |
| combined | 17.3 / 40.9 | 4.70 / 12.6 | 22.7 / 39.4 | 31.4 / 61.7 | 5.32 / 13.1 | 23.6 / 53.3 |
| combined-merged | 3.79 / 8.58 | 2.79 / 6.60 | 5.01 / 8.30 | 7.27 / 13.4 | 3.44 / 7.11 | 3.92 / 6.90 |

### Step time, Native (VM), 1 thread, lockstep, sleeping disabled (ms per tick, median / p95)

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---:|---:|---:|---:|---:|---:|
| ships | 18.2 / 36.8 | 2.91 / 5.95 | 36.3 / 69.9 | 43.3 / 71.8 | 1.52 / 3.94 | 34.9 / 63.9 |
| debris | 4.05 / 15.8 | 3.44 / 13.0 | 8.11 / 15.9 | 12.6 / 32.1 | 6.09 / 17.7 | 7.32 / 16.2 |
| combined | 43.2 / 86.4 | 13.3 / 36.4 | 70.6 / 154 | 82.9 / 159 | 11.8 / 27.5 | 57.3 / 119 |
| combined-merged | 10.6 / 31.8 | 8.09 / 27.2 | 11.1 / 30.0 | 19.3 / 49.8 | 9.67 / 26.8 | 10.1 / 26.7 |

### Step time, Native (VM), multi-thread (ms per tick, median / p95)

| Scenario | rapier | rapier-compound | avian | avian-compound | box2d-c |
|---|---:|---:|---:|---:|---:|
| ships | 8.86 / 28.9 | 1.11 / 4.01 | 27.4 / 74.2 | 0.60 / 3.94 | 11.8 / 31.6 |
| ships-merged | 0.74 / 2.13 | 0.18 / 0.48 | 2.56 / 7.94 | 0.78 / 1.97 | 0.83 / 1.63 |
| debris | 2.69 / 4.20 | 2.33 / 6.41 | 9.80 / 22.9 | 5.63 / 19.2 | 2.24 / 6.76 |
| raycasts | 14.0 / 42.6 | 5.46 / 21.6 | 55.7 / 106 | 6.76 / 25.9 | 13.3 / 40.2 |
| breakage | 10.1 / 51.9 | 1.08 / 3.81 | 27.6 / 67.6 | 0.52 / 2.32 | 8.95 / 22.8 |
| breakage-merged | 0.87 / 2.29 | 0.23 / 0.50 | 1.74 / 3.29 | 0.40 / 1.00 | 0.35 / 0.58 |
| asteroids | 0.10 / 0.35 | 0.11 / 0.28 | 0.69 / 2.29 | 0.54 / 1.83 | 0.09 / 0.19 |
| combined | 11.8 / 30.1 | 1.98 / 6.51 | 22.6 / 53.6 | 4.06 / 14.5 | 9.84 / 21.5 |
| combined-merged | 2.49 / 13.2 | 1.37 / 5.53 | 6.56 / 13.6 | 2.82 / 11.8 | 1.84 / 5.44 |

### Step time, wasm in Node (V8), 1 thread (ms per tick, median / p95)

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound |
|---|---:|---:|---:|---:|---:|
| ships | 8.11 / 19.4 | 0.99 / 2.39 | 14.0 / 27.6 | 21.8 / 33.9 | 0.07 / 0.23 |
| ships-merged | 0.36 / 0.60 | 0.07 / 0.11 | 0.64 / 0.84 | 1.16 / 1.98 | 0.04 / 0.10 |
| debris | 1.45 / 2.77 | 1.03 / 2.17 | 2.38 / 3.86 | 6.06 / 13.7 | 2.01 / 4.13 |
| raycasts | 10.5 / 33.0 | 2.76 / 7.47 | 33.0 / 80.6 | 42.6 / 74.6 | 3.06 / 10.9 |
| breakage | 8.14 / 19.4 | 1.11 / 2.63 | 12.5 / 22.2 | 24.9 / 44.1 | 0.13 / 0.37 |
| breakage-merged | 0.48 / 0.94 | 0.12 / 0.26 | 0.80 / 1.43 | 1.48 / 2.69 | 0.08 / 0.20 |
| asteroids | 0.10 / 0.20 | 0.10 / 0.20 | 0.09 / 0.17 | 0.40 / 0.82 | 0.23 / 0.45 |
| combined | 13.8 / 30.6 | 2.95 / 7.70 | 26.6 / 52.1 | 36.3 / 75.7 | 5.83 / 14.9 |
| combined-merged | 4.71 / 12.1 | 3.03 / 8.92 | 5.51 / 15.9 | 11.4 / 21.3 | 4.23 / 11.2 |

### Whole tick (split + step + raycasts), ms, median / p95 / max

| Platform | Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---|---:|---:|---:|---:|---:|---:|
| st | raycasts | 20.2 / 103 / 204 | 12.2 / 32.4 / 97.0 | 38.6 / 141 / 2306 | 55.4 / 124 / 283 | 8.57 / 27.1 / 80.1 | 44.9 / 94.6 / 252 |
| st | breakage | 11.0 / 30.7 / 88.5 | 0.92 / 4.02 / 14.4 | 21.0 / 55.0 / 383 | 30.3 / 55.0 / 206 | 0.10 / 0.66 / 22.4 | 25.6 / 45.3 / 156 |
| st | breakage-merged | 0.41 / 0.85 / 8.85 | 0.10 / 0.21 / 4.71 | 0.94 / 1.75 / 16.1 | 1.41 / 4.32 / 20.8 | 0.06 / 0.20 / 6.59 | 0.84 / 1.64 / 16.0 |
| st | combined | 31.6 / 63.7 / 134 | 11.9 / 28.8 / 123 | 57.9 / 95.0 / 274 | 52.4 / 114 / 263 | 6.71 / 15.4 / 73.2 | 60.9 / 109 / 258 |
| st | combined-merged | 11.5 / 29.0 / 100 | 9.27 / 23.6 / 72.6 | 14.7 / 32.5 / 119 | 18.7 / 38.9 / 94.6 | 10.7 / 24.7 / 61.1 | 12.9 / 26.2 / 80.5 |
| lockstep | raycasts | 14.1 / 24.2 / 53.7 | 6.71 / 12.2 / 56.9 | 25.4 / 41.9 / 110 | 30.9 / 44.2 / 69.2 | 7.17 / 13.4 / 65.2 | 26.1 / 40.8 / 104 |
| lockstep | breakage | 8.20 / 16.0 / 35.3 | 1.23 / 3.01 / 8.75 | 15.6 / 24.8 / 53.1 | 19.4 / 36.0 / 119 | 0.51 / 1.53 / 4.88 | 15.6 / 26.7 / 49.4 |
| lockstep | breakage-merged | 0.28 / 0.63 / 6.27 | 0.07 / 0.18 / 1.29 | 0.51 / 0.75 / 1.93 | 0.80 / 1.39 / 5.29 | 0.07 / 0.21 / 3.24 | 0.50 / 0.73 / 2.75 |
| lockstep | combined | 21.1 / 49.6 / 153 | 9.08 / 20.9 / 56.7 | 26.8 / 50.3 / 199 | 36.3 / 78.2 / 327 | 10.5 / 23.5 / 124 | 28.4 / 66.0 / 260 |
| lockstep | combined-merged | 6.91 / 12.4 / 43.3 | 6.14 / 11.3 / 42.8 | 8.59 / 12.7 / 28.1 | 10.8 / 17.7 / 46.6 | 7.14 / 11.8 / 20.7 | 8.02 / 12.3 / 32.7 |
| lockstep-nosleep | combined | 53.8 / 107 / 271 | 25.0 / 60.6 / 380 | 84.2 / 188 / 694 | 98.5 / 196 / 513 | 25.3 / 55.2 / 186 | 71.7 / 150 / 625 |
| lockstep-nosleep | combined-merged | 16.8 / 46.0 / 95.6 | 15.1 / 44.0 / 161 | 19.0 / 50.2 / 124 | 27.2 / 69.4 / 384 | 18.3 / 45.0 / 134 | 18.5 / 45.6 / 159 |
| mt | raycasts | 20.8 / 54.4 / 142 | 12.2 / 35.3 / 103 | – | 67.0 / 143 / 441 | 12.8 / 39.3 / 121 | 18.4 / 53.2 / 258 |
| mt | breakage | 10.1 / 51.9 / 128 | 1.09 / 3.82 / 36.0 | – | 28.1 / 76.5 / 162 | 0.53 / 2.55 / 8.36 | 9.01 / 23.3 / 118 |
| mt | breakage-merged | 0.87 / 2.29 / 19.3 | 0.23 / 0.52 / 8.37 | – | 1.75 / 3.63 / 50.2 | 0.40 / 1.05 / 5.80 | 0.35 / 0.60 / 11.2 |
| mt | combined | 17.0 / 40.9 / 164 | 5.75 / 12.8 / 81.8 | – | 28.5 / 75.1 / 146 | 9.76 / 25.9 / 96.8 | 15.9 / 31.9 / 165 |
| mt | combined-merged | 6.64 / 19.1 / 44.6 | 4.84 / 11.0 / 50.8 | – | 10.7 / 19.3 / 92.9 | 6.51 / 18.3 / 81.0 | 6.75 / 13.4 / 102 |
| node | raycasts | 15.3 / 41.6 / 94.8 | 7.29 / 15.0 / 64.3 | 37.9 / 96.6 / 287 | 49.2 / 83.1 / 227 | 7.46 / 20.6 / 42.7 | – |
| node | breakage | 8.16 / 19.4 / 47.6 | 1.11 / 2.64 / 12.3 | 12.5 / 22.6 / 56.9 | 25.1 / 49.8 / 133 | 0.13 / 0.43 / 8.60 | – |
| node | breakage-merged | 0.48 / 0.94 / 8.41 | 0.12 / 0.26 / 0.77 | 0.80 / 1.43 / 4.85 | 1.48 / 2.80 / 12.3 | 0.08 / 0.23 / 9.13 | – |
| node | combined | 21.4 / 40.4 / 91.7 | 10.1 / 18.8 / 37.9 | 33.0 / 62.5 / 125 | 44.4 / 94.6 / 300 | 16.8 / 32.4 / 77.6 | – |
| node | combined-merged | 13.5 / 24.2 / 62.5 | 12.3 / 21.2 / 136 | 11.4 / 27.8 / 88.2 | 19.7 / 34.6 / 128 | 12.2 / 22.5 / 57.8 | – |

### Raycasts: 5000 closest-hit rays per tick, ms per batch (median / p95), hits per run

| Platform | Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---|---:|---:|---:|---:|---:|---:|
| st | raycasts | 3.86 / 30.3 (1710761) | 5.28 / 17.1 (1710537) | 4.16 / 30.1 (1708034) | 5.02 / 25.7 (1708251) | 4.25 / 15.4 (1708061) | 4.97 / 18.5 (1708034) |
| st | combined | 7.26 / 21.8 (2706206) | 7.18 / 19.3 (2708105) | 8.68 / 24.0 (2668718) | 6.71 / 23.6 (2700631) | 4.17 / 8.70 (2702332) | 9.76 / 30.5 (2668718) |
| st | combined-merged | 6.12 / 14.4 (2705030) | 6.12 / 15.4 (2704086) | 6.91 / 16.6 (2656491) | 5.81 / 13.7 (2700528) | 5.72 / 13.6 (2701154) | 6.72 / 16.2 (2656491) |
| lockstep | raycasts | 2.10 / 3.95 (1710761) | 2.52 / 4.51 (1710537) | 2.51 / 4.82 (1708034) | 2.65 / 5.82 (1708251) | 2.99 / 5.20 (1708061) | 2.63 / 5.37 (1708034) |
| lockstep | combined | 3.65 / 10.1 (2706206) | 4.22 / 10.4 (2708105) | 4.11 / 10.1 (2668718) | 4.30 / 14.2 (2700631) | 5.01 / 12.6 (2702332) | 4.49 / 13.7 (2668718) |
| lockstep | combined-merged | 2.98 / 4.96 (2705030) | 3.34 / 5.90 (2704086) | 3.50 / 5.44 (2656491) | 3.34 / 5.04 (2700528) | 3.55 / 5.56 (2701154) | 3.79 / 5.53 (2656491) |
| lockstep-nosleep | combined | 8.31 / 30.0 (2706311) | 9.69 / 31.5 (2708609) | 11.7 / 45.2 (2668716) | 11.4 / 52.4 (2700420) | 11.8 / 33.0 (2702180) | 11.0 / 36.0 (2668716) |
| lockstep-nosleep | combined-merged | 5.72 / 21.3 (2705079) | 6.61 / 25.4 (2704137) | 7.21 / 24.4 (2656489) | 7.01 / 26.2 (2700396) | 7.66 / 24.4 (2701004) | 7.66 / 23.4 (2656489) |
| mt | raycasts | 5.18 / 16.6 (1710761) | 6.14 / 20.3 (1710537) | – | 7.63 / 45.6 (1708251) | 5.76 / 17.5 (1708061) | 4.12 / 20.1 (1708034) |
| mt | combined | 4.35 / 12.7 (2706206) | 3.71 / 7.56 (2708105) | – | 4.74 / 18.3 (2700631) | 5.42 / 13.6 (2702332) | 5.33 / 13.3 (2668718) |
| mt | combined-merged | 3.98 / 6.66 (2705030) | 3.32 / 5.90 (2704086) | – | 3.71 / 6.99 (2700528) | 3.39 / 7.27 (2701154) | 4.66 / 8.89 (2656491) |
| node | raycasts | 3.86 / 10.4 (1710761) | 4.28 / 8.37 (1710537) | 3.93 / 18.5 (1708034) | 5.03 / 13.5 (1708251) | 4.10 / 11.1 (1708061) | – |
| node | combined | 6.65 / 12.1 (2706206) | 6.72 / 12.7 (2708105) | 5.21 / 13.2 (2668718) | 6.48 / 18.1 (2700631) | 10.3 / 20.6 (2702332) | – |
| node | combined-merged | 8.25 / 14.5 (2705030) | 8.81 / 14.9 (2704086) | 4.72 / 13.3 (2656491) | 7.43 / 14.0 (2700528) | 7.54 / 12.7 (2701154) | – |

### Breakage: cost of one split event, ms (mean / max over the 30 events)

| Platform | Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---|---:|---:|---:|---:|---:|---:|
| st | breakage | 0.61 / 1.46 | 0.76 / 3.43 | 2.56 / 10.6 | 44.9 / 115 | 1.50 / 7.00 | 3.83 / 13.9 |
| st | breakage-merged | 0.05 / 0.28 | 0.11 / 2.21 | 0.13 / 0.39 | 1.51 / 5.77 | 0.18 / 0.72 | 0.14 / 0.39 |
| st | combined | 0.90 / 2.35 | 0.70 / 2.58 | 3.64 / 13.8 | 44.2 / 100 | 1.38 / 5.53 | 4.63 / 22.4 |
| st | combined-merged | 0.11 / 0.63 | 0.19 / 0.62 | 0.23 / 1.16 | 2.88 / 11.5 | 1.04 / 4.10 | 0.20 / 1.15 |
| lockstep | breakage | 0.45 / 1.08 | 0.44 / 1.41 | 1.89 / 5.92 | 27.0 / 82.0 | 0.86 / 3.49 | 2.67 / 6.09 |
| lockstep | breakage-merged | 0.03 / 0.10 | 0.04 / 0.14 | 0.05 / 0.15 | 0.47 / 1.14 | 0.14 / 0.58 | 0.06 / 0.17 |
| lockstep | combined | 0.65 / 1.51 | 0.44 / 0.99 | 2.22 / 8.00 | 30.0 / 77.2 | 1.47 / 4.21 | 3.78 / 13.9 |
| lockstep | combined-merged | 0.10 / 0.17 | 0.11 / 0.36 | 0.22 / 0.69 | 1.51 / 3.77 | 0.86 / 5.42 | 0.27 / 0.79 |
| lockstep-nosleep | combined | 1.53 / 6.72 | 1.17 / 3.72 | 8.34 / 53.5 | 81.3 / 197 | 3.99 / 13.3 | 7.88 / 27.6 |
| lockstep-nosleep | combined-merged | 0.27 / 0.57 | 0.32 / 0.91 | 0.66 / 2.46 | 4.85 / 21.2 | 1.88 / 8.83 | 0.67 / 3.08 |
| mt | breakage | 0.76 / 3.00 | 0.62 / 1.79 | – | 49.1 / 119 | 1.24 / 4.43 | 3.49 / 14.6 |
| mt | breakage-merged | 0.06 / 0.25 | 0.07 / 0.17 | – | 1.10 / 2.51 | 0.28 / 1.46 | 0.09 / 0.25 |
| mt | combined | 0.47 / 1.05 | 0.45 / 1.65 | – | 33.7 / 68.6 | 1.62 / 6.51 | 2.98 / 10.6 |
| mt | combined-merged | 0.08 / 0.20 | 0.11 / 0.35 | – | 1.70 / 4.96 | 0.58 / 3.01 | 0.14 / 0.37 |
| node | breakage | 0.71 / 2.20 | 0.67 / 1.61 | 2.19 / 6.85 | 39.8 / 99.4 | 0.73 / 1.92 | – |
| node | breakage-merged | 0.07 / 0.31 | 0.05 / 0.10 | 0.11 / 0.73 | 1.07 / 2.65 | 0.34 / 2.24 | – |
| node | combined | 0.64 / 1.66 | 0.73 / 2.28 | 2.72 / 8.10 | 43.0 / 110 | 1.99 / 5.72 | – |
| node | combined-merged | 0.15 / 0.56 | 0.33 / 0.98 | 0.34 / 1.18 | 2.84 / 7.67 | 1.31 / 7.16 | – |

### Setup: time to create all bodies and colliders, ms (median)

| Platform | Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---|---:|---:|---:|---:|---:|---:|
| st | ships | 11.3 | 18.2 | 32.2 | 526 | 19.5 | 45.3 |
| st | ships-merged | 1.95 | 0.98 | 5.45 | 7.78 | 3.26 | 3.00 |
| st | combined | 37.3 | 30.5 | 79.0 | 802 | 37.7 | 52.6 |
| lockstep | ships | 14.6 | 11.7 | 34.0 | 626 | 12.0 | 34.9 |
| lockstep | ships-merged | 0.20 | 0.40 | 0.91 | 4.45 | 0.68 | 0.88 |
| lockstep | combined | 6.25 | 7.40 | 17.9 | 448 | 21.8 | 25.3 |
| lockstep-nosleep | ships | 11.9 | 10.6 | 34.5 | 625 | 20.1 | 37.6 |
| lockstep-nosleep | combined | 38.4 | 32.1 | 81.5 | 1778 | 80.1 | 87.2 |
| mt | ships | 32.1 | 11.9 | – | 809 | 13.7 | 57.8 |
| mt | ships-merged | 1.24 | 2.31 | – | 13.3 | 3.56 | 4.07 |
| mt | combined | 15.2 | 14.5 | – | 513 | 33.5 | 50.1 |
| node | ships | 11.1 | 11.0 | 45.2 | 417 | 12.4 | – |
| node | ships-merged | 0.74 | 0.62 | 1.52 | 10.3 | 2.70 | – |
| node | combined | 12.3 | 11.6 | 40.8 | 871 | 67.8 | – |

### Native vs wasm (Node), runs back to back: step median native / Node (ms), ratio Node ÷ native

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound |
|---|---:|---:|---:|---:|---:|
| ships-merged | 0.66 / 0.97 (×1.58) | 0.10 / 0.26 (×2.77) | 1.00 / 1.36 (×1.36) | 1.51 / 2.81 (×1.87) | 0.06 / 0.16 (×3.23) |
| debris | 2.10 / 3.42 (×1.63) | 1.43 / 2.37 (×1.94) | 4.08 / 5.47 (×1.34) | 8.81 / 15.0 (×1.59) | 3.29 / 7.82 (×1.65) |
| combined | 34.1 / 35.6 (×1.00) | 4.02 / 6.45 (×1.60) | 66.2 / 81.7 (×1.35) | 58.4 / 60.6 (×1.34) | 4.58 / 6.15 (×1.39) |
| combined-merged | 3.74 / 6.93 (×1.75) | 3.39 / 4.64 (×1.44) | 5.61 / 6.17 (×1.16) | 14.7 / 23.7 (×1.61) | 4.47 / 6.78 (×1.55) |

### Determinism (final running state hash over all ticks)

Columns: same hash on every repeat within the platform; native 1-thread == Node wasm; native multi-thread == native 1-thread; lockstep binary (all engines linked together) == per-engine binary.

| Engine | Scenario | native repeats | Node repeats | native == Node | MT == ST | lockstep == ST | native hash |
|---|---|---|---|---|---|---|---|
| rapier | ships | yes (3) | yes (3) | yes | yes | yes | `60418ee60372a7e8` |
| rapier | ships-merged | yes (3) | yes (3) | yes | yes | yes | `5d7901af282e8421` |
| rapier | debris | yes (3) | yes (3) | yes | yes | yes | `5ad22995a546d274` |
| rapier | raycasts | yes (3) | yes (3) | yes | yes | yes | `5e5bc617e4d8f929` |
| rapier | breakage | yes (3) | yes (3) | yes | yes | yes | `dd01f584ff4fdcb5` |
| rapier | breakage-merged | yes (3) | yes (3) | yes | yes | yes | `0a37e01b72bdd888` |
| rapier | asteroids | yes (3) | yes (3) | yes | yes | yes | `49f1f9cdbdb2e3f1` |
| rapier | combined | yes (3) | yes (3) | yes | yes | yes | `08e14ecb94a1640a` |
| rapier | combined-merged | yes (3) | yes (3) | yes | yes | yes | `54e48401588ac7be` |
| rapier-compound | ships | yes (3) | yes (3) | yes | yes | yes | `163265885791b0e1` |
| rapier-compound | ships-merged | yes (3) | yes (3) | yes | yes | yes | `deb29a2e2f85c907` |
| rapier-compound | debris | yes (3) | yes (3) | yes | yes | yes | `01eaf3c06e581902` |
| rapier-compound | raycasts | yes (3) | yes (3) | yes | yes | yes | `d3316b4297bad060` |
| rapier-compound | breakage | yes (3) | yes (3) | yes | yes | yes | `6321e5b0b7d749cc` |
| rapier-compound | breakage-merged | yes (3) | yes (3) | yes | yes | yes | `4f09507fffcec987` |
| rapier-compound | asteroids | yes (3) | yes (3) | yes | yes | yes | `a54d41ece67ece6e` |
| rapier-compound | combined | yes (3) | yes (3) | yes | yes | yes | `7c2787261a238ba0` |
| rapier-compound | combined-merged | yes (3) | yes (3) | yes | yes | yes | `107325dd8eaded84` |
| box2d-rust | ships | yes (3) | yes (3) | yes | – | yes | `9bbceb11e61cc0c5` |
| box2d-rust | ships-merged | yes (3) | yes (3) | yes | – | yes | `17e67317479f7557` |
| box2d-rust | debris | yes (3) | yes (3) | yes | – | yes | `f3ef4d8f3c8d2cc1` |
| box2d-rust | raycasts | yes (3) | yes (3) | yes | – | yes | `fbe3fe26890917c4` |
| box2d-rust | breakage | yes (3) | yes (3) | yes | – | yes | `74db3c05b262ac20` |
| box2d-rust | breakage-merged | yes (3) | yes (3) | yes | – | yes | `b3365680c89e5064` |
| box2d-rust | asteroids | yes (3) | yes (3) | yes | – | yes | `8745d27d30ac07bb` |
| box2d-rust | combined | yes (3) | yes (3) | yes | – | yes | `9d42931569ef3f3c` |
| box2d-rust | combined-merged | yes (3) | yes (3) | yes | – | yes | `68c9613150f3539e` |
| avian | ships | yes (3) | yes (3) | yes | yes | yes | `0e9e531ed932b182` |
| avian | ships-merged | yes (3) | yes (3) | yes | yes | yes | `15cb013a44d98988` |
| avian | debris | yes (3) | yes (3) | yes | yes | yes | `d09deea3991b8dd2` |
| avian | raycasts | yes (3) | yes (3) | yes | yes | yes | `1652dbebbf9c81e9` |
| avian | breakage | yes (3) | yes (3) | yes | yes | yes | `c444276aa80d7067` |
| avian | breakage-merged | yes (3) | yes (3) | yes | yes | yes | `884e1140332e2293` |
| avian | asteroids | yes (3) | yes (3) | yes | yes | yes | `fe87febfceb0b712` |
| avian | combined | yes (3) | yes (3) | yes | yes | yes | `022ad346ce88ca68` |
| avian | combined-merged | yes (3) | yes (3) | yes | yes | yes | `d3fc89fc4075f69f` |
| avian-compound | ships | yes (3) | yes (3) | yes | yes | yes | `5071600a9a43b2dc` |
| avian-compound | ships-merged | yes (3) | yes (3) | yes | yes | yes | `58e602ed370be9ce` |
| avian-compound | debris | yes (3) | yes (3) | yes | yes | yes | `e8ee07e88b180299` |
| avian-compound | raycasts | yes (3) | yes (3) | yes | yes | yes | `331fc4f6bb9149c5` |
| avian-compound | breakage | yes (3) | yes (3) | yes | yes | yes | `c66867d4560e669a` |
| avian-compound | breakage-merged | yes (3) | yes (3) | yes | yes | yes | `b3131006989e5fc2` |
| avian-compound | asteroids | yes (3) | yes (3) | yes | yes | yes | `14983b313df6a750` |
| avian-compound | combined | yes (3) | yes (3) | yes | yes | yes | `10aa50f355f64153` |
| avian-compound | combined-merged | yes (3) | yes (3) | yes | yes | yes | `4b25350f2f0eddb0` |
| box2d-c | ships | yes (3) | – | – | yes | yes | `9bbceb11e61cc0c5` |
| box2d-c | ships-merged | yes (3) | – | – | yes | yes | `17e67317479f7557` |
| box2d-c | debris | yes (3) | – | – | yes | yes | `f3ef4d8f3c8d2cc1` |
| box2d-c | raycasts | yes (3) | – | – | yes | yes | `fbe3fe26890917c4` |
| box2d-c | breakage | yes (3) | – | – | yes | yes | `74db3c05b262ac20` |
| box2d-c | breakage-merged | yes (3) | – | – | yes | yes | `b3365680c89e5064` |
| box2d-c | asteroids | yes (3) | – | – | yes | yes | `8745d27d30ac07bb` |
| box2d-c | combined | yes (3) | – | – | yes | yes | `9d42931569ef3f3c` |
| box2d-c | combined-merged | yes (3) | – | – | yes | yes | `68c9613150f3539e` |

box2d-rust vs box2d-c (native, 1 thread): identical state hash in 9 of 9 scenarios.
box2d-c multi-thread vs 1 thread: identical in 9 of 9 scenarios.

Raycast result hash (hit body + hit distance bits), native vs Node:

- rapier / raycasts: native 41f2259f400401b0 · Node 41f2259f400401b0 → match
- rapier / combined: native 1f505bc3ec095cf1 · Node 1f505bc3ec095cf1 → match
- rapier-compound / raycasts: native 35d3fcfe13ef97c1 · Node 35d3fcfe13ef97c1 → match
- rapier-compound / combined: native a5011d87c8c2c258 · Node a5011d87c8c2c258 → match
- box2d-rust / raycasts: native 7c4ed92ac98b1b19 · Node 7c4ed92ac98b1b19 → match
- box2d-rust / combined: native b8c60ce82c20584c · Node b8c60ce82c20584c → match
- avian / raycasts: native 728edd5d97d7fa28 · Node 728edd5d97d7fa28 → match
- avian / combined: native aa278b2e2dadc3c7 · Node aa278b2e2dadc3c7 → match
- avian-compound / raycasts: native c82263150d4392a2 · Node c82263150d4392a2 → match
- avian-compound / combined: native 98f6616b3e4e15f1 · Node 98f6616b3e4e15f1 → match

