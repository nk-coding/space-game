# ADR-0008: Local git only, no remote repositories

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0007](0007-development-setup-vm-and-host-browser.md), `CLAUDE.md` "Hard rules", "Git conventions"

## Context

A personal hobby project, "closed for now". Claude Code writes and runs the code in Niklas's VM.

## Options considered

- **A hosted repository (GitHub, GitLab)**: not wanted for now. The only recorded reason is that the project is
  closed for now.
- **A local folder with local git** (chosen).

## Decision

- Local git in the VM only. Never create, push to or link a GitHub/GitLab repository.
- Work happens on branch `dev`; Niklas merges into `main` (from 11 Oct 2026, `CLAUDE.md`).
- Several agents share one working tree, so each commits only its own paths (never `git add -A`).

## Consequences

- There is no hosted CI. Anything the concept calls a CI run (the short balance run on every data change,
  GDR-0032) has to run locally.
