#!/usr/bin/env python3
"""PreToolUse hook for the Bash tool: reject commands that can wait forever.

Agents in this repo got stuck for hours on polling loops (see CLAUDE.md, "Wait loops must end"):
- `until ! pgrep -f "<pattern>"; do sleep ..; done` never ends, because `pgrep -f` also matches the
  shell running the loop (its own command line contains the pattern);
- `until grep -q <done-marker> build.log; do sleep ..; done` never ends when the build or server that
  writes the log has died or never prints that marker.

Rules:
0. No `git commit -a/--all`, `git add -A/--all` or `git add .` (shared working tree).
1. A polling loop (`until`/`while` together with `sleep`) must be bounded by `timeout <secs>`.
2. `pgrep -f` must not be used inside a polling loop at all.

Reads the hook JSON from stdin; prints a PreToolUse deny decision or nothing.
"""

import json
import re
import sys

LOOP = re.compile(r"\b(until|while)\b")
SLEEP = re.compile(r"\bsleep\b")
TIMEOUT = re.compile(r"\btimeout\s+(-\S+\s+)*\d+[smh]?\b")
PGREP_F = re.compile(r"\bpgrep\s+(-\w*\s+)*-\w*f")
# Several agents share one working tree: committing "everything" sweeps up their unfinished work.
GIT_COMMIT_ALL = re.compile(r"\bgit\s+(-C\s+\S+\s+)?commit\b[^;&|]*\s(-[a-zA-Z]*a[a-zA-Z]*|--all)\b")
QUOTED = re.compile(r'"(?:[^"\\]|\\.)*"|\'[^\']*\'')
GIT_ADD_ALL = re.compile(r"\bgit\s+(-C\s+\S+\s+)?add\s+(-A\b|--all\b|\.(\s|$|;|&))")


def deny(reason: str) -> None:
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }))


def main() -> None:
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        return
    command = (payload.get("tool_input") or {}).get("command") or ""
    unquoted = QUOTED.sub("''", command)  # commit messages may legitimately contain "-a"
    if GIT_COMMIT_ALL.search(unquoted) or GIT_ADD_ALL.search(unquoted):
        deny(
            "Blocked by scripts/hooks/guard-bash.py: several agents share this working tree, so "
            "`git commit -a/--all`, `git add -A/--all` and `git add .` would sweep up other agents' "
            "unfinished work. Stage your own paths explicitly (`git add <paths>`, or `git add -p`) and "
            "commit with `git commit -m ... -- <paths>`. See CLAUDE.md, 'Git conventions'."
        )
        return
    is_poll_loop = bool(LOOP.search(command) and SLEEP.search(command))
    if not is_poll_loop:
        return
    if PGREP_F.search(command):
        deny(
            "Blocked by scripts/hooks/guard-bash.py: `pgrep -f` inside a polling loop matches the loop's "
            "own shell (its command line contains the pattern), so the loop never ends. Wait on a PID "
            "without a loop (`timeout <secs> tail --pid=$PID -f /dev/null`), or run the command in the "
            "foreground with `timeout <secs> <cmd>`. See CLAUDE.md, 'Wait loops must end'."
        )
        return
    if not TIMEOUT.search(command):
        deny(
            "Blocked by scripts/hooks/guard-bash.py: polling loops (until/while + sleep) must be bounded. "
            "Wrap the loop as `timeout <secs> bash -c 'until <cond>; do sleep 5; done'`, and make <cond> "
            "match success AND failure. Better: avoid polling; run the build/check in the foreground with "
            "`timeout <secs> <cmd>`, wait for a PID with `timeout <secs> tail --pid=$PID -f /dev/null`, or "
            "use scripts/web-check.sh for web builds. See CLAUDE.md, 'Wait loops "
            "must end'."
        )


if __name__ == "__main__":
    main()
