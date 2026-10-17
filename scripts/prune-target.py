#!/usr/bin/env python3
"""Frees disk in the shared `target/`: cargo keeps every old build of a workspace crate, test or example under a new
hash, so `target/debug/deps` grows by gigabytes a day on a busy VM.

Usage: scripts/prune-target.py [--dry-run] [PROFILE ...]   (default profile: debug)

In `target/<PROFILE>/deps` and `target/<PROFILE>/examples` it keeps, for each artifact name (`<name>-<hash>.<ext>`),
the files of the newest hash and removes the older ones: the libraries of the workspace's own crates (their names
from `crates/*/*/Cargo.toml`) and every executable (tests and examples). Dependencies' libraries are left alone (two
hashes of a dependency are often two feature sets or versions in use at once). It also empties
`target/<PROFILE>/incremental` (the VM builds with `CARGO_INCREMENTAL=0`). The next build of a pruned crate relinks
from what is kept (or from sccache); nothing in use goes. Run it when no build is running.
"""
import glob
import os
import re
import shutil
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def workspace_crates():
    names = set()
    for manifest in glob.glob(os.path.join(ROOT, "crates", "*", "*", "Cargo.toml")):
        with open(manifest) as f:
            m = re.search(r'^name\s*=\s*"([^"]+)"', f.read(), re.M)
        if m:
            names.add(m.group(1).replace("-", "_"))
    return names


def prune_dir(d, crates, dry):
    """Removes the older hashes of each artifact name in `d`; returns the bytes freed."""
    groups = defaultdict(lambda: defaultdict(list))
    executables = set()
    for f in os.listdir(d):
        p = os.path.join(d, f)
        stem = f.split(".")[0]
        if not os.path.isfile(p) or "-" not in stem:
            continue
        name, h = stem.rsplit("-", 1)
        groups[name][h].append(p)
        if f == stem and os.access(p, os.X_OK):
            executables.add(name)
    freed = 0
    for name, hashes in groups.items():
        base = name[3:] if name.startswith("lib") else name
        if len(hashes) < 2 or (base not in crates and name not in executables):
            continue
        newest = max(hashes, key=lambda h: max(os.path.getmtime(p) for p in hashes[h]))
        for h, paths in hashes.items():
            if h == newest:
                continue
            for p in paths:
                freed += os.path.getsize(p)
                if not dry:
                    os.remove(p)
    return freed


def dir_size(d):
    return sum(os.path.getsize(os.path.join(r, f)) for r, _, fs in os.walk(d) for f in fs)


def main():
    args = sys.argv[1:]
    dry = "--dry-run" in args
    profiles = [a for a in args if a != "--dry-run"] or ["debug"]
    crates = workspace_crates()
    verb = "removable" if dry else "removed"
    for profile in profiles:
        base = os.path.join(ROOT, "target", profile)
        for sub in ("deps", "examples"):
            d = os.path.join(base, sub)
            if os.path.isdir(d):
                print(f"target/{profile}/{sub}: {prune_dir(d, crates, dry) / 1e9:.2f} GB {verb}")
        inc = os.path.join(base, "incremental")
        if os.path.isdir(inc):
            freed = dir_size(inc)
            if not dry:
                for e in os.listdir(inc):
                    shutil.rmtree(os.path.join(inc, e), ignore_errors=True)
            print(f"target/{profile}/incremental: {freed / 1e9:.2f} GB {verb}")


if __name__ == "__main__":
    main()
