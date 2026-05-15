#!/usr/bin/env bash
# Spawn all configured agent roles in parallel. Skips any role that's already
# registered with a valid on-disk session (idempotent per meridian-spawn.sh).
#
# Usage: script/meridian-spawn-all.sh [--force]
#
# Manager is intentionally excluded — Manager is the one running this script.
# A separate `meridian-spawn.sh manager` exists for when you want a headless
# Manager session (e.g., scheduled tasks).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
force_arg=""
if [[ "${1:-}" == "--force" ]]; then
    force_arg="--force"
fi

roles=("builder-1" "builder-2" "builder-3" "laniakea")

echo "spawning ${#roles[@]} agents in parallel..."
pids=()
for role in "${roles[@]}"; do
    "${SCRIPT_DIR}/meridian-spawn.sh" $force_arg "$role" &
    pids+=("$!")
done

# Wait for all + collect exit codes
fail=0
for pid in "${pids[@]}"; do
    if ! wait "$pid"; then
        fail=1
    fi
done

if [[ "$fail" -eq 0 ]]; then
    echo
    echo "all agents spawned. Registry:"
    for role in "${roles[@]}"; do
        sid_file="${HOME}/.meridian/agents/${role}/session-id"
        if [[ -f "$sid_file" ]]; then
            echo "  ${role}: $(<"$sid_file")"
        fi
    done
else
    echo "one or more spawns failed; check output above" >&2
    exit 1
fi
