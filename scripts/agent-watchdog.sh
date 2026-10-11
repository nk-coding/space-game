#!/usr/bin/env bash
# Watchdog for the lead session: finds agent processes that are probably stuck and exits as soon as it finds
# one, printing the details (so the lead, which ran it in the background, gets notified and can act).
# It never kills anything itself. Checks once a minute:
#   - polling loops (a shell whose command contains until/while + sleep) running longer than LOOP_MIN minutes;
#   - processes whose working directory was deleted (e.g. a server started from a removed worktree);
#   - cargo/trunk processes running longer than BUILD_MIN minutes (hung build or stuck on a lock);
#   - less than DISK_MIN_GB free on /home.
# Usage: scripts/agent-watchdog.sh [max_hours=6]
LOOP_MIN=${LOOP_MIN:-20} BUILD_MIN=${BUILD_MIN:-45} DISK_MIN_GB=${DISK_MIN_GB:-3}
deadline=$(( $(date +%s) + ${1:-6} * 3600 ))
self=$$

check() {
  local found=0 pid et cmd cwd
  for d in /proc/[0-9]*; do
    pid=${d#/proc/}
    [ "$pid" = "$self" ] && continue
    cmd=$(tr '\0' ' ' 2>/dev/null < "$d/cmdline") || continue
    [ -z "$cmd" ] && continue
    et=$(ps -o etimes= -p "$pid" 2>/dev/null | tr -d ' ') || continue
    [ -z "$et" ] && continue
    cwd=$(readlink "$d/cwd" 2>/dev/null)
    case "$cmd" in
      *bash\ -c*)
        if [ "$et" -gt $(( LOOP_MIN * 60 )) ] && echo "$cmd" | grep -qE '\b(until|while)\b' \
           && echo "$cmd" | grep -qE '\bsleep\b'; then
          echo "STUCK? polling loop pid=$pid running $(( et / 60 )) min cwd=$cwd"
          echo "  $(echo "$cmd" | sed "s/.*eval '//" | cut -c1-400)"
          found=1
        fi ;;
    esac
    if [[ "$cwd" == *"(deleted)"* ]] && [ "$et" -gt 120 ]; then
      echo "ORPHAN? pid=$pid runs from a deleted directory ($cwd), $(( et / 60 )) min: $(echo "$cmd" | cut -c1-200)"
      found=1
    fi
    case "$cmd" in
      *cargo\ *|*trunk\ *)
        if [ "$et" -gt $(( BUILD_MIN * 60 )) ] && [[ "$cmd" != *"trunk serve"* ]]; then
          echo "SLOW? pid=$pid build running $(( et / 60 )) min cwd=$cwd: $(echo "$cmd" | cut -c1-200)"
          found=1
        fi ;;
    esac
  done
  local free_gb
  free_gb=$(df -BG --output=avail /home | tail -1 | tr -dc '0-9')
  if [ "$free_gb" -lt "$DISK_MIN_GB" ]; then
    echo "DISK: only ${free_gb} GB free on /home"
    found=1
  fi
  return $(( 1 - found ))
}

while [ "$(date +%s)" -lt "$deadline" ]; do
  if check; then
    echo "watchdog: found the above at $(date '+%H:%M'); exiting so the lead can act"
    exit 0
  fi
  sleep 60
done
echo "watchdog: nothing stuck for ${1:-6} h; exiting"
