# ADR-0041: The web build keeps its documents in IndexedDB, read into memory at start and written behind; the local storage moves in once; lab runs are stored in parts

- Status: Accepted (provisional, lead)
- Date: 2026-10-16
- Decided by: Niklas (IndexedDB instead of the local storage, after a lab run filled it: "for what does indexeddb
  exist"); the shape by the lead session and the storage agent, for Niklas to review
- Amends: [ADR-0031](0031-player-storage-and-design-sharing.md)'s web backend (the rest of it stands)
- Related: [ADR-0031](0031-player-storage-and-design-sharing.md), [GDR-0092](../gdr/0092-evolution-lab-for-your-own-ships.md)
  "Persistence", `docs/client.md` "Storage and sharing", `docs/arena.md` "What a run keeps", `docs/drydock.md`
  "Storage and sharing"

## Context

ADR-0031 put the web build's documents in the browser's local storage: synchronous and simple, but about 5 MB per
origin, held as UTF-16. The evolution lab (GDR-0092) wrote each run as one document after every generation (about
0.7 MB of text for a STANDARD run at the end), so a few runs filled it: "NOT SAVED: THE STORAGE IS FULL; THE RUN IS
PAUSED". IndexedDB holds a share of the disk instead (Chromium: up to most of the free disk per origin), but answers
only asynchronously, while the game's `Storage` and every caller of it are synchronous (ADR-0031's `Backend` trait).

## Options considered

- **Keep local storage, compress harder or cap runs**: still 5 MB for designs, saves and runs together; no.
- **An IndexedDB crate** (`rexie`, `indexed_db_futures`, `idb`): more API than the four calls needed and another
  dependency; `web-sys`'s IndexedDB bindings and `wasm-bindgen-futures` (already in the build through Bevy) are enough.
- **Make `Storage` asynchronous**: every screen that saves would need to wait and handle late errors; no.
- **IndexedDB behind a synchronous cache** (chosen): read every document into memory before the app is built, keep
  writes in memory at once, commit them behind the calls; failures come back with the next write.
- **Lab runs**: one document rewritten after every generation (writes grow with the run); **a head plus one part
  per generation** (chosen): a generation writes its own bytes.

## Decision

- **Backend**: database `space-game`, store `docs` (keys `namespace/name`, values the stored text as before), store
  `meta` (the move's mark). `main` awaits `world::storage::preload()` before it builds the app: it opens the database,
  moves the local storage in (below) and reads every document into memory. Reads and lists come from memory.
- **Write behind**: a write or delete changes memory at once and marks its key; a microtask at the end of the frame's
  task commits what changed in one read-write transaction (deletes first, in their own), each key with its text as
  it is then, so the last write wins and the commit order is the order of the calls. A failed transaction (the quota's
  `QuotaExceededError`) leaves its keys unsaved: they go out again with the next write that deletes nothing, and
  right after any transaction that commits (a delete goes first on its own, so the room it makes is there). While the
  last transaction failed, every write returns that failure (`StorageError::Full` and the words the UI already had),
  deletes never (they make room). `Storage::unsaved` names a document whose write failed: the lab pauses a run whose checkpoint the
  storage didn't keep, as it does when a write fails at once.
- **Leaving the page**: on `pagehide` and on `visibilitychange` to hidden, the documents kept for leaving
  (`Storage::keep_on_leave`, as before) are written and every change in memory is committed right away, with
  `IDBTransaction.commit()`. A transaction committed before the page went away is finished by the browser
  (checked in Chromium on a reload); a crash or a killed tab loses what had not committed, at most the last
  frame's writes (and, as before, the lab's open generation).
- **The move from the local storage**, once, before the game first runs on IndexedDB: every key `space-game/<key>`
  is copied in one transaction, read back and compared; only then is the move marked done in `meta`, and only
  after that are the old keys removed, each only where IndexedDB holds the same text. Closed at any point, the next
  start finds it as it was: unmarked, it copies again (nothing newer can be in IndexedDB, since the game never ran on
  it); marked, it removes what is left. An old key whose text differs after the move (a tab of an older build still
  writing) is left untouched and named in the console. If IndexedDB doesn't open or the copy doesn't read back, the
  game runs on the old documents in memory, unsaved, with a note, and the next start tries again.
- **Persistent storage**: `navigator.storage.persist()` at start, best effort. The browser gives `navigator.storage`
  (and its `estimate()`) only to secure addresses (https, localhost): on `http://10.77.9.2:8080` neither exists, the
  storage still works.
- **Lab runs in parts** (`world::evolution::store`, `arena::RunParts`): `lab/<id>` is the head (format 2: name,
  created, status, the rules' fingerprint, the generations done, the best genome and the state between generations,
  packed; a few hundred bytes), `lab/<id>/config` the run's configuration (written once), `lab/<id>/g<n>` generation
  n's part (its numbers, its genomes and records, packed; written once when it closes). A generation writes its part
  and the head; parts go before the head, so a head never counts a part that isn't there (on the web they commit
  together anyway). A run read from its parts is the run its run file is (`Run::restore_parts`); the CLI keeps its
  one file, and IMPORT RUN splits it. A format-1 run (the whole file in `lab/<id>`) is written in parts when the lab
  loads. A document's parts are a storage feature: `namespace/name/part`, each segment a valid name, listed with
  `Storage::parts`; desktop keeps them as `<namespace>/<name>/<part>.ron`.
- **Usage**: the lab's RUNS page says what the storage holds and may hold (`navigator.storage.estimate()`), in
  coral with "NEARLY FULL: DELETE RUNS YOU DON'T NEED" under 64 MB or a tenth of the quota left (*placeholder*); on
  an insecure address the game's own bytes, without a quota. A full storage's message names deleting runs.

## Consequences

- The local storage is no longer written; the only code left for it reads and clears the old keys.
- Several tabs of the game on one origin each keep their own memory; the last commit of a document wins (as each
  tab's in-memory state did before).
- The storage's memory holds every document of the origin (a few MB; a DEEP run's parts a few MB more).
- `MAX_RUNS` (8) was chosen for the 5 MB; it stays a placeholder for Niklas to raise.
- Desktop's files backend is unchanged but for the parts' folders; Android's will be the same.
