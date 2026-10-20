# ADR-0048: The web build is a player platform, published on GitHub Pages

- Status: Accepted
- Date: 2026-10-20
- Decided by: Niklas
- Supersedes: [ADR-0006](0006-web-as-development-platform.md)
- Related: [ADR-0004](0004-target-platforms-and-performance-first.md), [ADR-0008](0008-local-git-only.md), [ADR-0034](0034-fast-everyday-web-builds-and-shared-compile-cache.md), [ADR-0041](0041-web-storage-in-indexeddb.md), [ADR-0043](0043-lab-web-workers.md), [ADR-0046](0046-one-dev-mode-replaces-the-url-flags.md), [ADR-0047](0047-licences-code-mit-or-apache-content-cc-by.md), `CLAUDE.md` "Current priorities", `docs/client.md` "Publishing the web build"

## Context

Since 11 Oct 2026 (ADR-0006) the web build was only Niklas's development platform: wasm size didn't matter, browser
performance only had to be comfortable, players would get Android and desktop. With the repository prepared for
publishing (ADR-0047), Niklas wants people to play the game in the browser without installing anything.

## Options considered

- **Keep the web build a development platform** (ADR-0006): reason not recorded beyond Niklas wanting a playable
  browser build now.
- **The web build is a player platform beside the native and Android builds, hosted on GitHub Pages** (chosen).

## Decision

- **The web build is a first-class player platform**, alongside the future native desktop and Android builds. It is
  hosted on **GitHub Pages** (a project page under `https://<user>.github.io/<repo>/`).
- **Deploy**: a GitHub Actions workflow (`.github/workflows/pages.yml`) builds the optimised web build (as
  `scripts/web-build.sh --full`: profile `release`, wasm-opt -O3, both wasm binaries) on every push to `main` (and by
  hand) and publishes it to Pages, with trunk's public URL set to `/<repo>/`. A second workflow (`ci.yml`) runs the
  workspace's tests and the wiki's build and check on pull requests and pushes. Both were written on 20 Oct 2026 and
  have not run yet: they run only where the repository is published.
- **The VM's repository keeps no remote** (ADR-0008 and `CLAUDE.md`'s hard rule stand; Niklas, 20 Oct 2026): the
  repository is published from elsewhere.
- **Dev mode** (`?dev=1`, ADR-0046) stays available in the public build; it plays its own save, so it never touches a
  player's career.
- **Priorities change**: wasm size and browser performance now matter for players; native performance still matters
  too. Everything stays Android-compatible and deterministic across native, wasm and Android.

## Consequences

- Pages can host the game as it is: the evolution lab's workers talk by messages, so no `SharedArrayBuffer` and no
  COOP/COEP headers are needed; saves are already in IndexedDB (ADR-0041).
- Paths: trunk writes the game's script and wasm under the public URL; the lab's worker loads
  `./lab-worker_loader.js` relative to the page, and that loader loads `./lab-worker.js` relative to itself, so all
  of it works under `/<repo>/`. The game fetches nothing else (content and icons are embedded).
- All project pages of one GitHub user share the origin `<user>.github.io`, so they share IndexedDB: the game's
  database is named `space-game`, which another project page of the same user must not reuse.
- Follow-ups, decided and not built yet: the trunk public URL for local checks of the `/<repo>/` subpath; wasm size
  and a loading screen; a browser performance pass; save export and import (a browser can wipe IndexedDB); a check in
  mobile browsers.
- ADR-0006's consequences that came from "development platform only" (size never optimised, browser numbers only
  for comfort) no longer hold; its others (Android later, wasm32 everywhere) carry over. Browser-only features that
  ADR-0006 dropped (mods from a zip or URL, WebRTC co-op) stay dropped unless Niklas brings them back.
