# ADR-0043: The web build runs the evolution lab's jobs in web workers: a second wasm binary `lab-worker`, jobs and results as RON messages, only the main thread breeds and writes

- Status: Accepted (provisional, lead)
- Date: 2026-10-17
- Decided by: lead session (GDR-0092 step 9, approved by Niklas; the shape is the plan's "web workers (optional,
  later)", for Niklas to review)
- Related: [ADR-0006](0006-web-as-development-platform.md), [ADR-0010](0010-cross-platform-determinism.md),
  [ADR-0034](0034-fast-everyday-web-builds-and-shared-compile-cache.md), [ADR-0036](0036-arena-crate.md), [ADR-0038](0038-workspace-structure-feature-folders-and-client-split.md),
  [ADR-0040](0040-arena-library-in-core.md), [ADR-0041](0041-web-storage-in-indexeddb.md),
  [GDR-0092](../gdr/0092-evolution-lab-for-your-own-ships.md), `docs/client.md` "Evolution", `docs/arena.md`
  "Jobs as messages"

## Context

The lab computes its runs at home and on every screen (GDR-0092). Natively a generation's jobs go to Bevy's compute
threads; on the web (single-threaded wasm) `arena::Run::step` ran in slices of the frame, 10 ms of it with the
EVOLUTION layer open and 4 ms elsewhere. That taxes every frame and is slow: in headless Chromium a QUICK generation
of the starter took about 48 s, 1.8 s of it in the frames. GDR-0092 kept web workers as an optional later step: a
second wasm binary and a heavier build. The determinism rule (ADR-0010) must hold: a run computed in workers must be
the run the frame's slices make, byte for byte.

## Options considered

- **A second wasm binary `lab-worker`, plain messages** (chosen): `crates/client/lab-worker` (`arena`, `data`, `sim`,
  wasm-bindgen; no Bevy), built by Trunk beside the game (`data-type="worker"`, its loader shim
  `lab-worker_loader.js`); jobs and results go as RON text; no shared memory, so no COOP/COEP headers and it works over
  plain `http://10.77.9.2`. Its cost: a second, small wasm build (7 MB in the web-dev profile, beside the game's 118
  MB) and a few seconds more per web build.
- **The game's own wasm in workers with shared memory (wasm threads)**: one binary, but needs `SharedArrayBuffer`
  (COOP/COEP headers, a secure context), nightly `build-std` with atomics, and Bevy's executor on it; far more build
  and serving work for the development platform.
- **Slices on the main thread only** (what was built in step 4): no second build, but the frame pays for the run.

## Decision

- **One executor path for both platforms** (`world::evolution::out`): the frame's budget breeds (`Run::breed`, as
  before), then the generation's jobs (`Run::jobs`, or a validation's or a rating's `arena::evolve::JobBatch`) go out
  four at a time to an `Executor`; `Out` takes the results in any order and hands them to the run in job order
  (`Run::submit`), which closes the generation as the slices would. Natively the executor is Bevy's compute threads
  (cores less one); on the web it is the workers.
- **The workers** (`world::evolution::workers`): `navigator.hardwareConcurrency` less one, started the first time the
  lab has work, each holding at most two chunks (the one it fights and the next, so it doesn't wait for a frame).
  A worker loads the base content and says `Ready` with its content key (`arena::worker::content_key`:
  `Content::fingerprint` and the lab's RON); one whose key differs from the game's is let go, and a task for other
  content is refused, so results of two contents never mix. A worker that fails hands its chunks back to the queue.
  With no worker (none loads, or `?lab_workers=0`) the run computes in the frame's slices as before; if every worker
  goes mid-generation, the slices do that generation again from its start (the same jobs, the same run).
- **Messages** (`arena::worker`): `Task { batch, first, content, jobs }` and `Reply::{Ready, Done, Refused}` as RON
  (RON writes floats exactly, so a result through a message is the result itself). The protocol and the worker's
  answer are plain functions in `arena`, tested natively.
- **Who writes**: only the main thread breeds, closes generations and writes the storage (ADR-0041's parts, the
  ratings' `ladder/<id>`); a worker only fights and never opens IndexedDB.
- **Deploying** the web build copies `lab-worker.js`, `lab-worker_bg.wasm` and `lab-worker_loader.js` (Trunk doesn't
  hash a worker's files) with the game's.

## Consequences

- The frame no longer pays for the run on the web: in headless Chromium (SwiftShader, about 4 frames a second, 4
  cores) a QUICK generation of the starter took 3.3–3.9 s with 30–50 ms of it in the frames (was 48 s and 1.8 s);
  an imported CLI run's generation parts computed in the workers and with `?lab_workers=0` are identical. The lab's
  log line per generation says how much of it the frames paid.
- `lab.ron`'s `budget` now bounds breeding and the slices' fallback; with workers a generation's time is the workers'.
- Every web build runs wasm-bindgen twice (the game, then the worker); the builds stay one after the other (~3.4 GB
  for the game's).
- Tests: the jobs out of the frame, through the worker's messages and on the native threads, make the run the
  slices make, file for file; a worker refuses other content; a rating is the same in slices and out of the frame.
