# ADR-0050: Saves are exported and imported as one RON backup file, in SETTINGS › SAVES; an import replaces

- Status: Accepted (provisional, lead)
- Date: 2026-10-20
- Decided by: lead session and the backup agent (defaults for Niklas to review), following ADR-0048's follow-up
- Related: [ADR-0048](0048-the-web-build-is-a-player-platform.md), [ADR-0041](0041-web-storage-in-indexeddb.md),
  [ADR-0031](0031-player-storage-and-design-sharing.md), [ADR-0044](0044-no-save-compatibility-before-release.md),
  [ADR-0046](0046-one-dev-mode-replaces-the-url-flags.md), `docs/client.md` "The web's storage"

## Context

Since ADR-0048 players play the web build on GitHub Pages. The browser keeps their documents in IndexedDB
(ADR-0041), which it may clear (site data cleared by the player, storage pressure when persistence wasn't granted,
another browser or device). Players need a way to keep a copy and bring it back. The storage holds named RON documents
in namespaces: `designs/`, `drydock/working` and `drydock/behavior` (the drafts), `behaviors/`, `saves/<slot>` (the
career, the dev mode's `saves/dev`), `settings/controls` and `settings/display`, the lab's runs `lab/<id>` with their
parts (a DEEP run is about 5 MB), and THE LADDER's ratings `ladder/<design id>`.

## Options considered

- **Format**: a zip or a packed code (small, but neither readable nor diffable, and a new dependency); one RON file per
  document in a folder (the browser downloads one file at a time); **one RON file, a map of key → the stored text in
  raw strings** (chosen: readable and diffable like the documents themselves, RON like the rest of the repo, one
  download).
- **What goes in**: only the career and designs (small, but loses behaviors, settings and the lab); **everything of a
  known kind** (chosen), with the lab's runs as a choice (WITH by default: they cost hours to breed; WITHOUT for a
  small file).
- **Where**: home's note block, a new home place, **a SAVES tab in SETTINGS** (chosen: settings is where "this
  device's" things already live, and the layer has room for a scrolling pane on a phone too).
- **Merge or replace**: merging per document (conflicts between two careers, designs that name behaviors that aren't
  there) or **replace** (chosen: the backup is the whole state; what the file left out stays).

## Decision

- **The file** (`world::backup`): `space-game-<save>-<yyyy-mm-dd-hhmm>.ron`, RON:
  `(backup: 1, game: "0.1.0", commit: Some("…") | None, exported: <Unix seconds>, date: "2026-10-20 14:03 UTC",
  save: Some("career") | None, left_out: ["lab"], documents: { "<key>": r#"<stored text>"#, … })`. Each document is
  the text as stored (its own header line with namespace, format and time, then its RON). `backup` is the file's own
  format; newer and older are refused. The commit comes from `SPACE_GAME_COMMIT` or GitHub Actions' `GITHUB_SHA` at
  build time (none in a local build).
- **What goes in**: every document of a known kind that reads in this build; the others (a newer format, a damaged
  one, a working draft set aside unread) are left out and named. Only the save the game is using goes in: the
  career's, or with `?dev=1` the dev mode's; the other save is never exported or touched. The lab's runs WITH
  (default) or WITHOUT (`left_out: ["lab"]`). THE LADDER's ratings always (small).
- **Import checks everything first**: the size (256 MB, *placeholder*), the backup format, every key against the
  known kinds and every document as the game reads it (header, format, RON); one save at most, the one the file names.
  A dev-mode save never becomes the career (import it with `?dev=1`); the career's save may become the dev mode's.
  Nothing is touched before the player sees what the file holds and what it replaces and confirms (REPLACE).
- **Replace, safely**: REPLACE first saves a copy of everything there is now (an export with the lab's runs,
  `…-before-import-….ron`); if that copy fails, nothing is replaced. Then the storage is frozen (the running game's
  writes are refused: its state is the old one), the file's documents are written, and only once they are kept (on
  the web: committed, in one IndexedDB transaction, so a failure leaves the stored saves as they were) are the
  documents of a known kind that the file doesn't hold deleted; the namespaces the file left out stay. Then the game
  starts again: the web reloads the page (without `newgame`), desktop asks the player to QUIT and start it again.
- **Platforms**: web: a Blob download and a hidden `<input type=file>` (the drydock's, now told what a file is picked
  for, so the lab's IMPORT RUN and SAVES never take each other's files); desktop: `<data dir>/exports/` and a file
  dropped onto the window (as for designs and run files). Nothing desktop-only in shared code (Android later: the
  same file backend).

## Consequences

- A player can keep and move everything; a wiped browser is one import away. The file is as large as the storage:
  with the lab's runs it may be tens of MB, which the web builds in memory (a string, then a Blob).
- No save compatibility before release (ADR-0044) holds for backups too: a backup of an older build's documents is
  refused where those documents would be.
- The settings over flight show SAVES off (home only), since a voyage never writes the career.
- Open: an automatic reminder to export (e.g. when the browser didn't grant persistent storage), and Android's file
  picker when Android lands.
