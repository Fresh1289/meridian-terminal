#!/usr/bin/env bash
# Claude Code SessionStart hook handler. Records the current session's UUID
# into ~/.meridian/agents/<role>/session-id so the Manager can later dispatch
# turns into this session via meridian-dispatch.sh.
#
# Usage (from .claude/settings.json hooks.SessionStart):
#   script/meridian-record-session.sh <role>
#
# Hook input arrives as JSON on stdin. Field is `session_id` per Claude Code
# hooks contract (verified against SessionStart payload shape).

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <role>" >&2
    exit 2
fi

role="$1"
session_id="$(jq -r '.session_id' </dev/stdin)"

if [[ -z "$session_id" || "$session_id" == "null" ]]; then
    echo "error: SessionStart hook input missing session_id" >&2
    exit 1
fi

dir="${HOME}/.meridian/agents/${role}"
mkdir -p "$dir"
printf '%s' "$session_id" > "${dir}/session-id"
