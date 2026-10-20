# Space Game

*(a working title)*

A 2D physical ship-builder. You are an awakened ship-mind in an abandoned precursor shipyard at the rim of a
procedural galaxy. You build ships block by block on a grid (no wiring: a shared power pool per piece, heat and
radiators, adjacency bonuses), decide how they think (behaviors on top of built-in manoeuvres and an always-on
flight computer), fly them yourself (stick and target lock) and salvage wrecks for materials and research. Ships are
fully physical (mass, centre of mass, thrust torque) and break apart.

Written in Rust with [Bevy](https://bevyengine.org/); the browser version is the same code compiled to WebAssembly.

## Status

A hobby project in development: playable, unbalanced and changing every day. Saves from older versions are not kept
([ADR-0044](docs/adr/0044-no-save-compatibility-before-release.md)).

The game will be playable in the browser on GitHub Pages, built from `main` by
[`.github/workflows/pages.yml`](.github/workflows/pages.yml) ([ADR-0048](docs/adr/0048-the-web-build-is-a-player-platform.md)).
Desktop and Android builds come later. `?dev=1` after the page's address opens a developer mode with its own save
(everything unlocked, the whole galaxy, debug keys).

## Build and run

You need Rust (stable, with the `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`),
[trunk](https://trunkrs.dev/) 0.21.14 for the web build and Node 24 for the wiki. The scripts are bash.

| What | Command |
|---|---|
| The game in the browser, with live reload | `scripts/web-serve.sh` → `http://localhost:8081` |
| The optimised web build (what GitHub Pages gets) into `crates/client/game/dist` | `scripts/web-build.sh --full` |
| All tests | `cargo test --workspace` |
| Determinism (native against wasm; needs the `wasm32-wasip1` target and Node) and the balance check | `scripts/determinism-check.sh`, `scripts/arena-check.sh` |
| A headless AI-vs-AI fight over 50 seeds (more: `docs/arena.md`) | `cargo run --release -p arena-cli -- fight crates/core/arena/fights/runner_vs_raiders.ron --seeds 50` |
| The wiki (generated from the game's data) | `cd wiki && npm install && npm run dev` |

More: [`docs/client.md`](docs/client.md) (the client, building and publishing), [`docs/sim.md`](docs/sim.md) (the
simulation), [`docs/arena.md`](docs/arena.md), [`docs/wiki.md`](docs/wiki.md),
[`docs/content-model.md`](docs/content-model.md) (the content and modding format).

## Design records

Every decision is written down: [`docs/adr/`](docs/adr/README.md) for architecture and technology,
[`docs/gdr/`](docs/gdr/README.md) for game design, [`docs/concept-notes.md`](docs/concept-notes.md) as the overview
and [`docs/roadmap.md`](docs/roadmap.md) for what is being built. [`CLAUDE.md`](CLAUDE.md) is the brief the AI agents
work from.

Some older records and results link to things public readers can't open: the local-only repositories
`../space-game-archive` (the original concept bundle, mockups and retired material) and
`../space-game-physics-bench` (the physics benchmark harness), which are not published, and a few private claude.ai
pages. The records summarise what those showed.

## How this is made

Made by Niklas with substantial AI assistance: AI agents (Anthropic's Claude, through Claude Code) write most of
the code, the documentation and the art (the vector icons and the look are generated as code and SVG), while Niklas
directs the work, makes the decisions and reviews the results. The decision records name who decided what.

The legal status of material generated purely by AI, including whether and by whom it can be copyrighted, is
unsettled and differs between jurisdictions. The licences below are offered as far as rights in the material exist.

## Licence

- **Code** (the Rust crates, scripts, tools, the wiki's code): MIT OR Apache-2.0, at your option
  ([`LICENSE-MIT`](LICENSE-MIT), [`LICENSE-APACHE`](LICENSE-APACHE)).
- **Content and art** (`content/` with its icons and lore, `art/tokens.json`, the docs and their screenshots, the
  wiki's pages): CC BY 4.0 ([`content/LICENSE`](content/LICENSE)).
- Fonts and other third-party material keep their own licences: [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

Which paths fall under which licence: [`LICENSE`](LICENSE) ([ADR-0047](docs/adr/0047-licences-code-mit-or-apache-content-cc-by.md)).
