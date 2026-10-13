# ADR-0031: A player storage layer of named, versioned RON documents; designs shared as files and share codes

- Status: Accepted (provisional, lead)
- Date: 2026-10-13
- Decided by: Niklas (a storage layer, plus a sharable format to share a design with others, GDR-0054); the
  technical shape by the lead session and the wave-2 storage agent, for Niklas to review
- Related: [GDR-0054](../gdr/0054-drydock-locked-parts-tools-saving-sharing.md), [ADR-0024](0024-saves-seed-and-deltas.md), [ADR-0030](0030-drydock-architecture.md), [ADR-0006](0006-web-as-development-platform.md), `docs/drydock.md` "Storage and sharing"

## Context

The drydock must keep designs across sessions and let players share them. Save games, behaviors and settings will
need the same kind of storage (ADR-0024: versioned, with migrations). `sim` and `data` stay free of I/O. The web
build is the development platform; desktop and Android are the end-user platforms.

## Options considered

- Storage: ad-hoc storage per feature; **one small backend trait under named, versioned documents** (chosen);
  IndexedDB on the web (asynchronous, unnecessary for documents of a few KB).
- Design format: **the content ship format** (chosen, ADR-0030); a separate save format.
- Sharing: files only; share codes only; **both** (chosen).
- Code encoding: raw RON (long, breaks in chat); **a version prefix plus base64url of zlib-compressed RON**
  (chosen).

## Decision

- `game::storage` stores named RON documents in namespaces (`designs/`, `drydock/` now; `saves/`, `behaviors/`,
  `settings/` later); names are `[a-z0-9_-]`. Each document starts with a header comment (namespace, format,
  saved time); a newer format is refused with a message, an older one is migrated by its document type.
- Backends: the browser's local storage on the web build; files in the user's data directory, written atomically
  (temp file and rename), on desktop (Android: the same files backend on the app's internal data path, later);
  memory for tests and as a fallback. Errors reach the UI.
- Saved designs are content ship RON in their drydock frame; the working design autosaves about a second after
  the last change and when a test flight starts, and is restored at start.
- Sharing: `.ron` export (the content ship format, trimmed) and share codes `SGD<version>:` + base64url of the
  zlib-compressed RON of the packs the design was made with and the ship. Imports are validated
  (`data::validate_ship`); missing content is named with the pack list.
- Copy and paste go through a small `Clipboard` resource per platform (copy/paste events on the web, which also
  work on the non-secure `http://10.77.9.2`; `arboard` on desktop).

## Consequences

- Save games, behaviors and settings reuse the layer and its versioning.
- Designs depend on stable content ids (already a rule).
- Browser storage is per origin: designs at `:8080` and `:8081` are separate; export or a share code moves them.
- Share codes stay valid across builds until the share format changes; pack versions are informational.
- New dependencies: `base64` and `miniz_oxide` (pure Rust); `arboard` on desktop.
- Not built yet: Android's storage root and clipboard, a native file dialog (desktop imports by drag and drop).
