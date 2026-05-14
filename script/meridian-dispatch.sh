#!/usr/bin/env bash
# Manager-side dispatch primitive. Appends a turn to a role's existing
# Claude Code session by running `claude --resume <id> --print` against
# the session-id recorded by meridian-record-session.sh.
#
# Usage: script/meridian-dispatch.sh <role> "<dispatch text>"
#   role: builder-1 | builder-2 | builder-3 | laniakea (etc.)
#
# Exits with claude's exit code; prints captured JSON to stdout.

set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 <role> \"<dispatch text>\"" >&2
    exit 2
fi

role="$1"
text="$2"

registry="${HOME}/.meridian/agents/${role}/session-id"
if [[ ! -f "$registry" ]]; then
    echo "error: no session-id registered for role '${role}' at ${registry}" >&2
    echo "hint: open ${role}'s claude session so the SessionStart hook fires" >&2
    exit 1
fi

session_id="$(<"$registry")"
if [[ -z "$session_id" ]]; then
    echo "error: registry file ${registry} is empty" >&2
    exit 1
fi

exec claude --resume "$session_id" --print "$text" --output-format json
