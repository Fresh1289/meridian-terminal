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

# Unified relay log: every dispatch + response appended here so a single
# watcher (`meridian-watch.sh relays`) can show the full orchestration
# timeline across all agents.
log_file="${HOME}/.meridian/relay-log.jsonl"
mkdir -p "$(dirname "$log_file")"
ts_start="$(date -u +%Y-%m-%dT%H:%M:%S.%NZ 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ)"
printf '{"event":"dispatch","time":"%s","role":"%s","session_id":"%s","text":%s}\n' \
    "$ts_start" "$role" "$session_id" "$(printf '%s' "$text" | jq -Rs .)" >> "$log_file"

# `--dangerously-skip-permissions` matches the posture of interactive Builder
# panes (which CTO enables system-wide). Without it, dispatched turns run
# with default permissions and can't use Bash/Edit/etc. without prompt-gating,
# which fails silently in non-interactive `--print` mode. Builder identity +
# Security Posture in CLAUDE.md still constrain behavior.
#
# Capture (not exec) so we can log the response too. Forward exit code.
set +e
response="$(claude --resume "$session_id" --print "$text" --output-format json --dangerously-skip-permissions)"
exit_code=$?
set -e

ts_end="$(date -u +%Y-%m-%dT%H:%M:%S.%NZ 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ)"
# `--arg/--argjson` lets jq inject the raw response object as a sub-field;
# falls back to a string envelope if response isn't valid JSON (e.g. claude error).
if printf '%s' "$response" | jq empty 2>/dev/null; then
    jq -nc \
        --arg time "$ts_end" --arg role "$role" --argjson exit_code "$exit_code" \
        --argjson response "$response" \
        '{event: "response", time: $time, role: $role, exit_code: $exit_code, response: $response}' \
        >> "$log_file"
else
    jq -nc \
        --arg time "$ts_end" --arg role "$role" --argjson exit_code "$exit_code" \
        --arg raw "$response" \
        '{event: "response", time: $time, role: $role, exit_code: $exit_code, raw_stdout: $raw}' \
        >> "$log_file"
fi

# Forward response to caller (preserves the existing dispatch contract).
printf '%s' "$response"
exit "$exit_code"
