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

role_dir="${HOME}/.meridian/agents/${role}"
registry="${role_dir}/session-id"
cwd_file="${role_dir}/cwd"

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

if [[ ! -f "$cwd_file" ]]; then
    echo "error: no cwd recorded for role '${role}' at ${cwd_file}" >&2
    echo "hint: reset ${role}'s claude session so the SessionStart hook records cwd alongside session-id" >&2
    exit 1
fi

project_cwd="$(<"$cwd_file")"
if [[ ! -d "$project_cwd" ]]; then
    echo "error: recorded cwd '${project_cwd}' for role '${role}' does not exist" >&2
    exit 1
fi

# `claude --resume <id>` scopes session lookup to the cwd's project — must
# match where the session was originally created. CD before exec.
cd "$project_cwd"
exec claude --resume "$session_id" --print "$text" --output-format json
