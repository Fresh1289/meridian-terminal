#!/usr/bin/env bash
# Headless spawn of an agent session. Creates a fresh claude session in the
# role's project cwd, fires the SessionStart hook (which records the new
# session-id + cwd into ~/.meridian/agents/<role>/), exits.
#
# Usage: script/meridian-spawn.sh <role>
#   role: builder-1 | builder-2 | builder-3 | laniakea | manager
#
# Idempotent: if the role is already registered AND its JSONL still exists,
# returns success without spawning. Use --force to spawn anyway.

set -euo pipefail

force=0
if [[ "${1:-}" == "--force" ]]; then
    force=1
    shift
fi

if [[ $# -ne 1 ]]; then
    echo "usage: $0 [--force] <role>" >&2
    echo "  role: builder-1 | builder-2 | builder-3 | laniakea | manager" >&2
    exit 2
fi

role="$1"

# Role → project cwd mapping
case "$role" in
    builder-1) project_cwd="${HOME}/meridian-warp-wt1" ;;
    builder-2) project_cwd="${HOME}/meridian-warp-wt2" ;;
    builder-3) project_cwd="${HOME}/meridian-warp-wt3" ;;
    laniakea)  project_cwd="${HOME}/laniakea" ;;
    manager)   project_cwd="${HOME}/meridian-warp" ;;
    *)
        echo "error: unknown role '${role}'" >&2
        echo "  known: builder-1 builder-2 builder-3 laniakea manager" >&2
        exit 1
        ;;
esac

if [[ ! -d "$project_cwd" ]]; then
    echo "error: project cwd '${project_cwd}' for role '${role}' does not exist" >&2
    exit 1
fi

registry_dir="${HOME}/.meridian/agents/${role}"
registry_file="${registry_dir}/session-id"

# Idempotency: skip if already registered + JSONL exists
if [[ "$force" -eq 0 && -f "$registry_file" ]]; then
    existing_id="$(<"$registry_file")"
    encoded="$(echo "$project_cwd" | sed 's|/|-|g')"
    jsonl_path="${HOME}/.claude/projects/${encoded}/${existing_id}.jsonl"
    if [[ -f "$jsonl_path" ]]; then
        echo "already registered: ${role} → ${existing_id}"
        echo "  (re-spawn with --force if you want a fresh session)"
        exit 0
    fi
fi

# Spawn fresh session. The wake prompt fires the FIRST MESSAGE RULE in the
# role's CLAUDE.md identity, putting the agent in its standard ready state.
echo "spawning ${role} in ${project_cwd}..."
cd "$project_cwd"

# Run in foreground so we wait for the SessionStart hook to complete + the
# claude session to be fully alive. --print exits when the agent's first
# response is done; by then the hook has written the registry.
claude --print "Wake. Confirm you are online and ready, then stand by." \
    --output-format json \
    --dangerously-skip-permissions \
    > /dev/null 2>&1 || true

# Verify the hook populated the registry (sometimes there's a small delay)
for _ in 1 2 3 4 5; do
    [[ -f "$registry_file" ]] && break
    sleep 1
done

if [[ ! -f "$registry_file" ]]; then
    echo "error: spawn completed but registry not populated at ${registry_file}" >&2
    echo "hint: check ${project_cwd}/.claude/settings.json has the SessionStart hook" >&2
    exit 1
fi

new_id="$(<"$registry_file")"
echo "spawned ${role} → ${new_id}"
